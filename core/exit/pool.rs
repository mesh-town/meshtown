//! Connections to network members: one per member (`PeerConnPool`, the service is set at creation), a stream
//! per request (`ask_exit_to_connect` — only for the exit pool).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::transport::net::{BoxRecv, BoxSend, NetStack, PeerConn, PeerId, ServiceId, StreamPair};
use anyhow::{anyhow, bail, Result};
use tokio::sync::{Mutex as AsyncMutex, OnceCell};
use tokio::time::timeout;

use crate::node::output::short_peer_id;

use super::ask_reply::read_exit_reply;
use super::frame::encode_exit_request;
use super::gateway_memo::GatewayMemo;
use super::traffic::{Direction, ExitTraffic};

/// A connection cell for one member: empty until the connection is open; whoever
/// arrives first fills it, others heading to the same member wait for it rather than for the shared map lock.
type Slot = Arc<OnceCell<Arc<dyn PeerConn>>>;

/// How long to wait for `open_bi` (the connection's stream budget is exhausted) before refusing the request.
pub const OPEN_STREAM_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a request waits for the member's cell to be filled (a connection being dialed, possibly by another
/// request) before it gives up; twice the transport's own connect limit, so a single dial ends first.
pub const CONNECT_WAIT: Duration = Duration::from_secs(20);

/// The `open_bi` deadline passed: the connection is alive, but all exit streams are busy. It is carried inside
/// `anyhow::Error`; the caller finds it via `downcast_ref` and tells "exit overloaded"
/// from "exit is not responding". The exit's own short reply (`ExitBusy`) is the second such sign.
#[derive(Debug)]
pub struct StreamBusy {
    /// Short exit id.
    pub peer: String,
    /// How many seconds `open_bi` was awaited.
    pub secs: u64,
    pub open_bi_waiter_count: usize,
}

impl std::fmt::Display for StreamBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "stream to {} didn't open in {} s — waiting for a stream: {}",
            self.peer, self.secs, self.open_bi_waiter_count
        )
    }
}

impl std::error::Error for StreamBusy {}

/// Connections to members: one per member, a stream per request.
/// Dials connections through the service set at creation. The map lock is held only to pick
/// the member's cell — a handshake with an unreachable member doesn't delay streams to the others.
#[derive(Clone)]
pub struct PeerConnPool {
    conns: Arc<AsyncMutex<HashMap<PeerId, Slot>>>,
    svc: ServiceId,
    /// How many pool tasks (of all clones) are currently waiting on `open_bi`.
    open_bi_waiter_count: Arc<AtomicUsize>,
    /// Deadline for one stream's `open_bi`; when it expires `open_peer_stream` returns an error.
    open_timeout: Duration,
    /// Deadline for filling the member's cell; when it expires the request returns an error.
    connect_wait: Duration,
    /// Speed registry: the pool's outgoing connections are counted as `ThroughPeer`; `None` — not counted.
    traffic: Option<ExitTraffic>,
    /// Exits that refused the UDP gateway target.
    gateway: GatewayMemo,
}

/// Guard of the waiting counter: +1 on creation, −1 on `Drop` (success, error, deadline, dropped future).
struct OpenBiWaiterGuard(Arc<AtomicUsize>);

impl OpenBiWaiterGuard {
    fn enter(counter: &Arc<AtomicUsize>) -> OpenBiWaiterGuard {
        counter.fetch_add(1, Ordering::SeqCst);
        OpenBiWaiterGuard(counter.clone())
    }
}

impl Drop for OpenBiWaiterGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Default for PeerConnPool {
    /// The exit protocol pool.
    fn default() -> PeerConnPool {
        PeerConnPool::new(crate::transport::net::service::EXIT)
    }
}

impl PeerConnPool {
    /// An empty pool whose connections are dialed through the service `svc`.
    pub fn new(svc: ServiceId) -> PeerConnPool {
        PeerConnPool {
            conns: Arc::default(),
            svc,
            open_bi_waiter_count: Arc::default(),
            open_timeout: OPEN_STREAM_TIMEOUT,
            connect_wait: CONNECT_WAIT,
            traffic: None,
            gateway: GatewayMemo::default(),
        }
    }

    /// Whether `peer` refused the UDP gateway target recently (then it is not asked again for a while).
    pub fn gateway_refused_recently(&self, peer: PeerId) -> bool {
        self.gateway
            .refused_recently(peer, std::time::Instant::now())
    }

    /// Notes that `peer` refused the UDP gateway target.
    pub fn note_gateway_refused(&self, peer: PeerId) {
        self.gateway
            .remember_refused(peer, std::time::Instant::now());
    }

    /// Turns on speed counting of the pool's outgoing connections in the `traffic` registry.
    pub fn with_traffic(mut self, traffic: ExitTraffic) -> PeerConnPool {
        self.traffic = Some(traffic);
        self
    }

    /// Replaces the `open_bi` deadline (tests; the node calls `PeerConnPool::default()`).
    pub fn with_open_timeout(mut self, limit: Duration) -> Self {
        self.open_timeout = limit;
        self
    }

    /// Replaces the cell-filling deadline (tests; the node calls `PeerConnPool::default()`).
    pub fn with_connect_wait(mut self, limit: Duration) -> Self {
        self.connect_wait = limit;
        self
    }

    pub fn open_bi_waiter_count(&self) -> usize {
        self.open_bi_waiter_count.load(Ordering::SeqCst)
    }

    /// Opens a stream to a member through the pool's service; a closed connection is reopened.
    /// `open` waits no longer than `open_timeout` (default `OPEN_STREAM_TIMEOUT`), then returns an error.
    pub async fn open_peer_stream(&self, stack: &NetStack, peer: PeerId) -> Result<StreamPair> {
        let (_, pair) = self.open_stream(stack, peer).await?;
        Ok(pair)
    }

    /// Like `open_peer_stream`, plus the `stable_id` of the connection the stream is on (for `forget`).
    /// A failed `open` drops the connection from the cell: the next request dials a new one.
    async fn open_stream(&self, stack: &NetStack, peer: PeerId) -> Result<(u64, StreamPair)> {
        let conn = self.cell_connection(stack, peer).await?;
        let id = conn.stable_id();
        let _open_bi_waiter = OpenBiWaiterGuard::enter(&self.open_bi_waiter_count);
        let pair = self.open_bi_in_time(&conn, peer, id).await?;
        Ok((id, pair))
    }

    /// Opens a stream on `conn` within `open_timeout`. A failed `open` drops the connection `id` from the cell;
    /// an expired deadline is a `StreamBusy` error (the connection stays).
    async fn open_bi_in_time(
        &self,
        conn: &Arc<dyn PeerConn>,
        peer: PeerId,
        id: u64,
    ) -> Result<StreamPair> {
        match timeout(self.open_timeout, conn.open()).await {
            Ok(Ok(pair)) => Ok(pair),
            Ok(Err(e)) => {
                self.forget(peer, id).await;
                Err(anyhow!(
                    "stream to {} didn't open: {e}",
                    short_peer_id(&peer)
                ))
            }
            Err(_) => Err(StreamBusy {
                peer: short_peer_id(&peer),
                secs: self.open_timeout.as_secs(),
                open_bi_waiter_count: self.open_bi_waiter_count(),
            }
            .into()),
        }
    }

    /// The member's connection: waits for the cell to be filled, but no longer than `connect_wait`.
    async fn cell_connection(&self, stack: &NetStack, peer: PeerId) -> Result<Arc<dyn PeerConn>> {
        let slot = self.slot(peer).await;
        let filled = slot.get_or_try_init(|| self.connect(stack, peer));
        match timeout(self.connect_wait, filled).await {
            Ok(conn) => Ok(conn?.clone()),
            Err(_) => bail!(
                "connection to {} didn't open: still not connected after {} s",
                short_peer_id(&peer),
                self.connect_wait.as_secs()
            ),
        }
    }

    /// Empties the member's cell if it still holds the connection with this `stable_id`. The connection is
    /// not closed: other streams may live on it.
    pub(crate) async fn forget(&self, peer: PeerId, stable_id: u64) {
        let mut map = self.conns.lock().await;
        let Some(slot) = map.get_mut(&peer) else {
            return;
        };
        if slot.get().is_some_and(|conn| conn.stable_id() == stable_id) {
            *slot = Slot::default();
        }
    }

    /// The member's cell: a closed connection is replaced with an empty cell. The map lock is held
    /// only inside this function and never touches the network.
    async fn slot(&self, peer: PeerId) -> Slot {
        let mut map = self.conns.lock().await;
        let slot = map.entry(peer).or_default();
        if slot.get().is_some_and(|conn| conn.is_closed()) {
            *slot = Slot::default();
        }
        slot.clone()
    }

    /// Opens a new connection to a member and starts its watcher. An error leaves the cell
    /// empty: the next request tries again.
    async fn connect(&self, stack: &NetStack, peer: PeerId) -> Result<Arc<dyn PeerConn>> {
        let conn = stack
            .connect(peer, self.svc)
            .await
            .map_err(|e| anyhow!("connection to {} didn't open: {e}", short_peer_id(&peer)))?;
        spawn_link_stats_log(conn.clone(), peer, self.svc);
        if let Some(traffic) = &self.traffic {
            traffic.watch(conn.clone(), Direction::ThroughPeer);
        }
        Ok(conn)
    }
}

/// Overload line tag for a pool by service: the `[exit-client]` line exists only for the exit pool.
fn link_stats_tag(svc: ServiceId) -> Option<&'static str> {
    (svc == crate::transport::net::service::EXIT).then_some("exit-client")
}

/// Starts a watcher for a new connection if this service's pool has a line.
fn spawn_link_stats_log(conn: Arc<dyn PeerConn>, peer: PeerId, svc: ServiceId) {
    if let Some(tag) = link_stats_tag(svc) {
        crate::node::output::spawn(super::report::log_link_stats_while_open(conn, peer, tag));
    }
}

/// Asks a member to open a connection (exit pool only); returns the stream and the reply code.
pub async fn ask_exit_to_connect(
    stack: &NetStack,
    pool: &PeerConnPool,
    peer: PeerId,
    target_host: &str,
    port: u16,
) -> Result<(BoxSend, BoxRecv, u8)> {
    use tokio::io::AsyncWriteExt;
    let (id, StreamPair { mut send, mut recv }) = pool.open_stream(stack, peer).await?;
    let frame = encode_exit_request(target_host, port)?;
    if let Err(e) = send.write_all(&frame).await {
        pool.forget(peer, id).await;
        bail!("request not sent: {e}");
    }
    let code = read_exit_reply(&mut recv, &peer).await?;
    Ok((send, recv, code))
}

#[cfg(test)]
#[path = "pool_ask_tests.rs"]
mod ask_tests;
#[cfg(test)]
#[path = "pool_reply_tests.rs"]
mod reply_tests;
#[cfg(test)]
#[path = "pool_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "pool_wait_tests.rs"]
mod wait_tests;
