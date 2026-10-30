//! The `hyper` transport: a townjump DHT node, a server with an announcement, and dialing by key.
//! One node both listens and dials. The session is built by `conn`, incoming ones are parsed by `accept`.

use crate::transport::net::addr::{PeerAddr, RelayRef};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::link::NetInfo;
use crate::transport::net::service::ServiceId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::transport::{PeerConn, Transport};
use crate::transport::net_hyper::accept::{AcceptCtx, Accepted};
use crate::transport::net_hyper::conn::establish_dialed_session;
use crate::transport::net_hyper::dht_table::TableSaver;
use crate::transport::net_hyper::gate::{Admit, PeerGate};
use crate::transport::net_hyper::link::{net_info, PathFacts};
use crate::transport::net_hyper::netchange::{spawn as spawn_netchange, DhtReact};
use crate::transport::net_hyper::records::HyperRecords;
use crate::transport::net_hyper::relay::RelayHost;
use crate::transport::net_hyperdht::RecordStore;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;
use townjump_dht::{Dht, DhtError, DhtServer, Duplex, KeyPair};
use townjump_dht_peeroxide::{PeeroxideDht, PeeroxideFactory, PeeroxideServer};

mod bind;
mod dialer;
mod factory;
mod held;
mod hints;
mod own_addr;
mod phases;
mod race;
mod relay;

#[cfg(test)]
mod connect_opts_tests;
#[cfg(test)]
mod direct_fallback_tests;
#[cfg(test)]
mod direct_tests;
#[cfg(test)]
mod path_tests;
#[cfg(test)]
mod relay_api_tests;
#[cfg(test)]
mod relayed_dial_tests;

use dialer::NodeDialer;
#[cfg(test)]
pub(crate) use relay::DialFaults;

/// Transport name in the config and in the registry.
pub const KIND: &str = "hyper";
/// Limit of one `connect`: lookup, hole punching, handshake and `Ack`.
const CONNECT_LIMIT: Duration = Duration::from_secs(10);
/// How long the direct phase of a dial with a relay candidate may take.
const DIRECT_LIMIT: Duration = Duration::from_secs(5);
/// How long the relay phase may take.
const RELAY_LIMIT: Duration = Duration::from_secs(8);
/// Limit of one `connect` that may take both phases and the `Ack`.
const RELAYED_CONNECT_LIMIT: Duration = Duration::from_secs(14);
/// How many incoming sessions wait for `accept`.
const ACCEPT_QUEUE: usize = 64;
/// How many dials run at once (fewer than the 16 slots of the townjump server queue).
const DIAL_SLOTS: usize = 8;

/// Node parameters: `None` — addresses from `TOWNJUMP_BOOTSTRAP` or the public townjump nodes.
#[derive(Clone, Debug, Default)]
pub struct HyperOpts {
    pub bootstrap: Option<Vec<String>>,
    /// Watch the OS network and survive its change (disabled in tests).
    pub watch_net: bool,
    /// File of the saved DHT table: read before the node starts, written while it runs and on close; `None` — off.
    pub table_file: Option<PathBuf>,
}

type ListenOutcome = Option<Result<(), String>>;

pub struct HyperTransport {
    me: PeerId,
    node: Arc<PeeroxideDht>,
    server: PeeroxideServer,
    gate: Arc<PeerGate>,
    ctx: Arc<AcceptCtx>,
    dialer: Arc<NodeDialer>,
    incoming: tokio::sync::Mutex<mpsc::Receiver<Accepted>>,
    listen_outcome: watch::Sender<ListenOutcome>,
    closed: watch::Sender<bool>,
    close_started: AtomicBool,
    saver: Option<TableSaver>,
    relay_host: Mutex<Option<RelayHost>>,
    relay_factory: PeeroxideFactory,
    relay_identity: KeyPair,
}

/// The limit of one `connect`: the relay phase needs more time, so it applies only when a source is set.
pub(crate) fn connect_limit(has_source: bool) -> Duration {
    match has_source {
        true => RELAYED_CONNECT_LIMIT,
        false => CONNECT_LIMIT,
    }
}

pub fn dht_error_to_net_error(e: DhtError) -> NetError {
    match e {
        DhtError::PeerNotFound => NetError::NoRoute,
        DhtError::Destroyed => NetError::Closed,
        DhtError::Io(io) => NetError::Io(io),
        other => NetError::Other(anyhow::anyhow!("{other}")),
    }
}

fn new_session() -> NetResult<u64> {
    let mut bytes = [0u8; 8];
    crate::members::network_key::fill_random(&mut bytes).map_err(NetError::Other)?;
    Ok(u64::from_be_bytes(bytes))
}

/// Server announcement; the outcome is in `ListenOutcome`.
async fn run_listen(state: watch::Sender<ListenOutcome>, server: PeeroxideServer, kp: KeyPair) {
    let res = server.listen(kp).await.map_err(|e| e.to_string());
    state.send_replace(Some(res));
}

impl HyperTransport {
    async fn watch_network(&self) {
        let react = Arc::new(DhtReact::new(self.node.clone(), self.ctx.clone()));
        spawn_netchange(react, self.closed.subscribe()).await
    }

    /// Port of the DHT socket, the one a peer's `connect_direct` targets.
    pub async fn local_port(&self) -> Option<u16> {
        self.node.local_port().await.ok()
    }

    /// Waits for the `listen` outcome: the announcement is made or the reason why not.
    pub async fn wait_listen_outcome(&self) -> NetResult<()> {
        let mut rx = self.listen_outcome.subscribe();
        let done = rx.wait_for(|state| state.is_some()).await;
        let state = done.map_err(|_| NetError::Closed)?.clone();
        state
            .unwrap_or(Ok(()))
            .map_err(|e| NetError::Other(anyhow::anyhow!(e)))
    }

    /// `Some(predicate)` — admit to services by it; `None` — everyone.
    pub fn set_admit(&self, admit: Option<Admit>) {
        self.gate.set_admit(admit);
    }

    /// Record store of the DHT on the same node that listens and dials.
    pub fn records(&self) -> Arc<dyn RecordStore> {
        Arc::new(HyperRecords {
            node: self.node.clone(),
        })
    }

    /// Refuses a dial after `close`, to ourselves; otherwise waits for dials to open and notes the hints.
    async fn check_dial_allowed(&self, to: &PeerAddr) -> NetResult<()> {
        if self.close_started.load(Ordering::SeqCst) {
            return Err(NetError::Closed);
        }
        let _ = timeout(CONNECT_LIMIT, self.ctx.dials_open()).await;
        if to.id == self.me {
            return Err(NetError::NoRoute);
        }
        self.add_hints(to);
        Ok(())
    }

    async fn connect_inner(&self, to: PeerAddr, svc: ServiceId) -> NetResult<Arc<dyn PeerConn>> {
        self.check_dial_allowed(&to).await?;
        let (ctrl, facts) = self.dialer.dial_with_path(to.id).await?;
        self.start_dialed_session(to.id, svc, ctrl, facts).await
    }

    async fn start_dialed_session(
        &self,
        to: PeerId,
        svc: ServiceId,
        ctrl: Box<dyn Duplex>,
        facts: PathFacts,
    ) -> NetResult<Arc<dyn PeerConn>> {
        let session = new_session()?;
        let stream_dialer = self.dialer.clone();
        let conn = establish_dialed_session(
            to,
            session,
            svc.as_bytes(),
            ctrl,
            stream_dialer,
            self.ctx.sessions.clone(),
        )
        .await?;
        conn.note_path(facts);
        self.ctx.track(&conn);
        Ok(conn)
    }

    #[cfg(test)]
    pub(crate) fn net_react(&self) -> DhtReact {
        DhtReact::new(self.node.clone(), self.ctx.clone())
    }

    async fn accept_inner(&self) -> Option<Accepted> {
        let mut closed = self.closed.subscribe();
        let mut rx = self.incoming.lock().await;
        tokio::select! {
            biased;
            _ = closed.wait_for(|c| *c) => None,
            item = rx.recv() => item,
        }
    }

    async fn close_inner(&self) {
        if self.close_started.swap(true, Ordering::SeqCst) {
            return;
        }
        self.ctx.close_sessions(0, b"transport closed");
        let _ = self.server.close().await;
        if let Some(saver) = &self.saver {
            saver.finish().await;
        }
        let host = self.host_slot().take();
        let stop_host = async {
            if let Some(host) = host {
                host.close().await;
            }
        };
        let _ = tokio::join!(stop_host, self.node.destroy());
        self.closed.send_replace(true);
    }
}

impl Transport for HyperTransport {
    fn kind(&self) -> &'static str {
        KIND
    }

    fn me(&self) -> PeerId {
        self.me
    }

    fn local_addr(&self) -> PeerAddr {
        PeerAddr {
            id: self.me,
            hints: own_addr::own_hints(&self.node.node_mode()),
        }
    }

    fn can_reach(&self, _to: &PeerAddr) -> bool {
        true
    }

    fn add_hints(&self, addr: &PeerAddr) {
        if addr.id != self.me {
            self.dialer.hints.note(addr);
        }
    }

    fn proven_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        self.dialer.hints.proven(peer).map(SocketAddr::V4)
    }

    fn set_relays(&self, _relays: &[RelayRef]) {}

    fn serve(&self, svcs: &[ServiceId]) {
        self.ctx.serve(svcs);
    }

    fn connect(
        &self,
        to: &PeerAddr,
        svc: ServiceId,
    ) -> BoxFuture<'_, NetResult<Arc<dyn PeerConn>>> {
        let to = to.clone();
        Box::pin(async move {
            timeout(
                connect_limit(self.dialer.relay.has_source()),
                self.connect_inner(to, svc),
            )
            .await
            .map_err(|_| NetError::Timeout)?
        })
    }

    fn accept(&self) -> BoxFuture<'_, Option<Accepted>> {
        Box::pin(self.accept_inner())
    }

    fn net_info(&self) -> NetInfo {
        let seen = self.listen_outcome.borrow().as_ref().map(|res| res.is_ok());
        net_info(seen)
    }

    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(self.close_inner())
    }
}
