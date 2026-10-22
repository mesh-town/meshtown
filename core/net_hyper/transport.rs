//! The `hyper` transport: a townjump DHT node, a server with an announcement, and dialing by key.
//! One node both listens and dials. The session is built by `conn`, incoming ones are parsed by `accept`.

use crate::net::addr::{PeerAddr, RelayRef};
use crate::net::error::{NetError, NetResult};
use crate::net::id::{NodeSecret, PeerId};
use crate::net::link::NetInfo;
use crate::net::service::ServiceId;
use crate::net::stream::BoxFuture;
use crate::net::transport::{PeerConn, Transport};
use crate::net_hyper::accept::{accept_loop, AcceptCtx, Accepted};
use crate::net_hyper::conn::{establish_dialed_session, StreamDialer};
use crate::net_hyper::gate::{Admit, PeerGate};
use crate::net_hyper::keys::{key_pair, peer_of};
use crate::net_hyper::link::net_info;
use crate::net_hyper::netchange::{spawn as spawn_netchange, DhtReact};
use crate::net_hyper::records::HyperRecords;
use crate::net_hyperdht::RecordStore;
use townjump_dht::{ConnectOpts, Dht, DhtError, DhtFactory, DhtServer, Duplex, KeyPair, ServerOpts};
use townjump_dht_peeroxide::{PeeroxideDht, PeeroxideFactory, PeeroxideServer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, Semaphore};
use tokio::time::timeout;

mod held;

use held::Held;

/// Transport name in the config and in the registry.
pub const KIND: &str = "hyper";
/// Limit of one `connect`: lookup, hole punching, handshake and `Ack`.
const CONNECT_LIMIT: Duration = Duration::from_secs(10);
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
}

type ListenOutcome = Option<Result<(), String>>;

pub struct HyperTransport {
    me: PeerId,
    node: Arc<PeeroxideDht>,
    server: PeeroxideServer,
    gate: Arc<PeerGate>,
    ctx: Arc<AcceptCtx>,
    incoming: tokio::sync::Mutex<mpsc::Receiver<Accepted>>,
    listen_outcome: watch::Sender<ListenOutcome>,
    closed: watch::Sender<bool>,
    close_started: AtomicBool,
}

/// Dials DHT connections of the node and checks the other side's key.
struct NodeDialer {
    node: Arc<PeeroxideDht>,
    /// Simultaneous dials: the incoming queue of the townjump server has 16 slots, a larger burst drops connections.
    slots: Arc<Semaphore>,
}

pub fn dht_error_to_net_error(e: DhtError) -> NetError {
    match e {
        DhtError::PeerNotFound => NetError::NoRoute,
        DhtError::Destroyed => NetError::Closed,
        DhtError::Io(io) => NetError::Io(io),
        other => NetError::Other(anyhow::anyhow!("{other}")),
    }
}

impl NodeDialer {
    async fn dial_checked(&self, to: PeerId) -> NetResult<Box<dyn Duplex>> {
        let slot = self.slots.clone().acquire_owned().await.map_err(|_| NetError::Closed)?;
        // reusable_socket: after the first hole punch the fork remembers the route, the next connect goes without it.
        let conn = self.node.connect(*to.as_bytes(), ConnectOpts { reusable_socket: true, ..Default::default() })
            .await
            .map_err(dht_error_to_net_error)?;
        let got = peer_of(&conn.remote_public_key)?;
        match got == to {
            true => Ok(Box::new(Held::new(conn.stream, slot))),
            false => Err(NetError::WrongPeer { want: to, got }),
        }
    }
}

impl StreamDialer for NodeDialer {
    fn dial(&self, to: PeerId) -> BoxFuture<'_, NetResult<Box<dyn Duplex>>> {
        Box::pin(async move { timeout(CONNECT_LIMIT, self.dial_checked(to)).await.map_err(|_| NetError::Timeout)? })
    }
}

fn new_session() -> NetResult<u64> {
    let mut bytes = [0u8; 8];
    crate::network_key::fill_random(&mut bytes).map_err(NetError::Other)?;
    Ok(u64::from_be_bytes(bytes))
}

/// Server announcement; the outcome is in `ListenOutcome`.
async fn run_listen(state: watch::Sender<ListenOutcome>, server: PeeroxideServer, kp: KeyPair) {
    let res = server.listen(kp).await.map_err(|e| e.to_string());
    state.send_replace(Some(res));
}

impl HyperTransport {
    /// Starts the node and the server; `listen` runs as a task (wait for its outcome with `wait_listen_outcome`).
    pub async fn bind(secret: NodeSecret, opts: HyperOpts) -> NetResult<Arc<Self>> {
        let kp = key_pair(&secret);
        let factory = match opts.bootstrap {
            Some(list) => PeeroxideFactory { bootstrap: Some(list), ..Default::default() },
            None => PeeroxideFactory::from_env(),
        };
        let node = Arc::new(factory.create(Some(kp.clone())).await.map_err(dht_error_to_net_error)?);
        let gate = Arc::new(PeerGate::open());
        // reusable_socket: same as townjump-server, so that clients follow the remembered route.
        let server_opts = ServerOpts { firewall: None, reusable_socket: true, ..Default::default() };
        let (server, incoming) = node.create_server(server_opts).map_err(dht_error_to_net_error)?;
        let (accepted_tx, out) = mpsc::channel(ACCEPT_QUEUE);
        let ctx = Arc::new(AcceptCtx::new(Arc::new(NodeDialer { node: node.clone(), slots: Arc::new(Semaphore::new(DIAL_SLOTS)) }), accepted_tx, gate.clone()));
        let this = Arc::new(Self {
            me: secret.peer_id(),
            node,
            server: server.clone(),
            gate,
            ctx: ctx.clone(),
            incoming: tokio::sync::Mutex::new(out),
            listen_outcome: watch::channel(None).0,
            closed: watch::channel(false).0,
            close_started: AtomicBool::new(false),
        });
        tokio::spawn(accept_loop(incoming, ctx));
        tokio::spawn(run_listen(this.listen_outcome.clone(), server, kp));
        if opts.watch_net { spawn_netchange(Arc::new(DhtReact::new(this.node.clone(), this.ctx.clone())), this.closed.subscribe()).await }
        Ok(this)
    }

    /// Waits for the `listen` outcome: the announcement is made or the reason why not.
    pub async fn wait_listen_outcome(&self) -> NetResult<()> {
        let mut rx = self.listen_outcome.subscribe();
        let done = rx.wait_for(|state| state.is_some()).await;
        let state = done.map_err(|_| NetError::Closed)?.clone();
        state.unwrap_or(Ok(())).map_err(|e| NetError::Other(anyhow::anyhow!(e)))
    }

    /// `Some(predicate)` — admit to services by it; `None` — everyone.
    pub fn set_admit(&self, admit: Option<Admit>) {
        self.gate.set_admit(admit);
    }

    /// Record store of the DHT on the same node that listens and dials.
    pub fn records(&self) -> Arc<dyn RecordStore> {
        Arc::new(HyperRecords { node: self.node.clone() })
    }

    async fn connect_inner(&self, to: PeerAddr, svc: ServiceId) -> NetResult<Arc<dyn PeerConn>> {
        if self.close_started.load(Ordering::SeqCst) {
            return Err(NetError::Closed);
        }
        let _ = timeout(CONNECT_LIMIT, self.ctx.dials_open()).await;
        if to.id == self.me {
            return Err(NetError::NoRoute);
        }
        let ctrl = self.ctx.dialer.dial(to.id).await?;
        let session = new_session()?;
        let stream_dialer = self.ctx.dialer.clone();
        let conn = establish_dialed_session(to.id, session, svc.as_bytes(), ctrl, stream_dialer, self.ctx.sessions.clone()).await?;
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
        let _ = self.node.destroy().await;
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
        PeerAddr { id: self.me, hints: vec![] }
    }

    fn can_reach(&self, _to: &PeerAddr) -> bool {
        true
    }

    fn add_hints(&self, _addr: &PeerAddr) {}

    fn set_relays(&self, _relays: &[RelayRef]) {}

    fn serve(&self, svcs: &[ServiceId]) {
        self.ctx.serve(svcs);
    }

    fn connect(&self, to: &PeerAddr, svc: ServiceId) -> BoxFuture<'_, NetResult<Arc<dyn PeerConn>>> {
        let to = to.clone();
        Box::pin(async move {
            timeout(CONNECT_LIMIT, self.connect_inner(to, svc)).await.map_err(|_| NetError::Timeout)?
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
