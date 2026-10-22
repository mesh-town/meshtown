//! Accepting incoming DHT connections: each one is parsed by its own task so that the townjump queue (16 slots)
//! does not wait for somebody else's header. `Open` creates a session and puts it into the `Transport::accept` queue,
//! `Stream` hands over a stream of an already live session.

use crate::net::id::PeerId;
use crate::net::service::ServiceId;
use crate::net::stream::StreamPair;
use crate::net::transport::PeerConn;
use crate::net_hyper::conn::{establish_accepted_session, HyperConn, StreamDialer};
use crate::net_hyper::gate::PeerGate;
use crate::net_hyper::keys::peer_of;
use crate::net_hyper::table::{SessionKey, Sessions};
use crate::net_hyper::wire::{read_header, write_reply, Header, Owner, Reply, HEADER_LIMIT, REFUSE_NOT_MEMBER, REFUSE_UNSERVED};
use townjump_dht::{DhtConnection, Duplex, Incoming};
use std::sync::{Arc, Mutex, PoisonError, RwLock, Weak};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

/// An accepted session and the service it was dialed to.
pub type Accepted = (ServiceId, Arc<dyn PeerConn>);

/// Shared state of accepting and dialing: services, the stream table, the `accept` queue, the list of live sessions.
pub struct AcceptCtx {
    pub sessions: Arc<Sessions>,
    pub dialer: Arc<dyn StreamDialer>,
    pub accepted_tx: mpsc::Sender<Accepted>,
    pub gate: Arc<PeerGate>,
    services: RwLock<Vec<ServiceId>>,
    live_conns: Mutex<Vec<Weak<HyperConn>>>,
    /// `true` — dials are open; closed while the network changes (`hold_dials`).
    dials: watch::Sender<bool>,
}

impl AcceptCtx {
    pub fn new(dialer: Arc<dyn StreamDialer>, accepted_tx: mpsc::Sender<Accepted>, gate: Arc<PeerGate>) -> Self {
        Self {
            sessions: Arc::new(Sessions::new()),
            dialer,
            accepted_tx,
            gate,
            services: RwLock::new(Vec::new()),
            live_conns: Mutex::new(Vec::new()),
            dials: watch::channel(true).0,
        }
    }

    /// Sets the set of served services (a repeated call replaces the previous one).
    pub fn serve(&self, svcs: &[ServiceId]) {
        *self.services.write().unwrap_or_else(PoisonError::into_inner) = svcs.to_vec();
    }

    fn find(&self, name: &[u8]) -> Option<ServiceId> {
        let services = self.services.read().unwrap_or_else(PoisonError::into_inner);
        services.iter().copied().find(|s| s.as_bytes() == name)
    }

    /// Remembers the session by a weak reference: the last handle still closes it.
    pub fn track(&self, conn: &Arc<HyperConn>) {
        let mut live_conns = self.live_conns.lock().unwrap_or_else(PoisonError::into_inner);
        live_conns.retain(|w| w.strong_count() > 0);
        live_conns.push(Arc::downgrade(conn));
    }

    /// Closes every live session and forgets the table; returns the number of closed live sessions.
    pub fn close_sessions(&self, code: u32, reason: &[u8]) -> usize {
        let live_conns = std::mem::take(&mut *self.live_conns.lock().unwrap_or_else(PoisonError::into_inner));
        let alive: Vec<_> = live_conns.iter().filter_map(Weak::upgrade).collect();
        for conn in &alive {
            conn.close(code, reason);
        }
        self.sessions.close_all();
        alive.len()
    }

    /// Closes (`true`) or opens (`false`) the dial gate while the network changes.
    pub(crate) fn hold_dials(&self, hold: bool) {
        self.dials.send_replace(!hold);
    }

    /// Waits until the dial gate is open.
    pub(crate) async fn dials_open(&self) {
        let _ = self.dials.subscribe().wait_for(|open| *open).await;
    }
}

/// Accepts connections until the townjump server is closed.
pub async fn accept_loop(mut incoming: Incoming, ctx: Arc<AcceptCtx>) {
    while let Some(conn) = incoming.accept().await {
        tokio::spawn(admit(conn, ctx.clone()));
    }
}

/// Parses the header of one connection; drops a foreign or slow one without a reply.
async fn admit(conn: DhtConnection, ctx: Arc<AcceptCtx>) {
    let key = conn.remote_public_key;
    let mut stream = conn.stream;
    let Ok(peer) = peer_of(&key) else { return };
    let Ok(Ok(header)) = timeout(HEADER_LIMIT, read_header(&mut stream)).await else { return };
    match header {
        Header::Open { session, service_name } => on_open(&ctx, peer, session, &service_name, stream).await,
        Header::Stream { session, owner } => on_stream(&ctx, peer, session, owner, stream).await,
    }
}

async fn on_open(ctx: &Arc<AcceptCtx>, peer: PeerId, session: u64, service_name: &[u8], mut stream: Box<dyn Duplex>) {
    let Some(svc) = ctx.find(service_name) else { return refuse(&mut stream, REFUSE_UNSERVED).await };
    if !ctx.gate.admits(peer, &svc) {
        return refuse(&mut stream, REFUSE_NOT_MEMBER).await;
    }
    let made = establish_accepted_session(peer, session, stream, ctx.dialer.clone(), ctx.sessions.clone()).await;
    let Ok(conn) = made else { return };
    ctx.track(&conn);
    let conn: Arc<dyn PeerConn> = conn;
    let _ = ctx.accepted_tx.send((svc, conn)).await;
}

/// Data stream: a slot in the session queue is taken before `Ack`, the stream itself goes after it.
async fn on_stream(ctx: &AcceptCtx, peer: PeerId, session: u64, owner: Owner, mut stream: Box<dyn Duplex>) {
    let key = SessionKey { peer, session, numbered_by_us: owner == Owner::Acceptor };
    let permit = match ctx.sessions.reserve(key) {
        Ok(permit) => permit,
        Err(code) => return refuse(&mut stream, code).await,
    };
    if write_reply(&mut stream, &Reply::Ack).await.is_err() || stream.flush().await.is_err() {
        return;
    }
    let (recv, send) = tokio::io::split(stream);
    permit.send(StreamPair { send: Box::new(send), recv: Box::new(recv) });
}

async fn refuse(stream: &mut Box<dyn Duplex>, code: u32) {
    let reason = "refused".as_bytes().to_vec();
    if write_reply(stream, &Reply::Refuse { code, reason }).await.is_ok() {
        let _ = stream.flush().await;
    }
    let _ = stream.shutdown().await;
}

#[cfg(test)]
#[path = "accept_tests.rs"]
mod accept_tests;
