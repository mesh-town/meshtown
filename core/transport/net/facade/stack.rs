//! Running facade: transport choice and connecting, hints, publishing, snapshot, closing.
//! Closing goes in one order: discovery, acceptor, handlers, transports.

use super::services::HandlerSet;
use crate::transport::net::addr::{PeerAddr, RelayRef};
use crate::transport::net::discovery_mux::{DiscoveryMux, MANUAL_SOURCE};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::service::ServiceId;
use crate::transport::net::transport::{PeerConn, Transport};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use tokio::task::JoinHandle;

pub(super) struct NetStackShared {
    pub(super) me: PeerId,
    pub(super) transports: Vec<Arc<dyn Transport>>,
    pub(super) mux: Arc<DiscoveryMux>,
    pub(super) handler_tasks: Arc<HandlerSet>,
    pub(super) tasks: Mutex<Vec<JoinHandle<()>>>,
    pub(super) closed: AtomicBool,
    pub(super) close_gate: tokio::sync::Mutex<bool>,
}

/// Safety net: a stack dropped without `close()` aborts its tasks at once and waits for nothing.
impl Drop for NetStackShared {
    fn drop(&mut self) {
        let tasks = self.tasks.get_mut().unwrap_or_else(PoisonError::into_inner);
        tasks.iter().for_each(|h| h.abort());
        self.handler_tasks.abort_all();
        if !self.closed.load(Ordering::SeqCst) {
            tracing::warn!(
                "NetStack dropped without close(): tasks aborted, transports closed without coordination"
            );
        }
    }
}

/// A stack is closed with `close().await`: it stops discovery, the acceptor, handlers and transports in order and waits for
/// each of them. Without `close()` the stack closes when the last clone goes away: the discovery, acceptor and handler tasks
/// are aborted at once, transports are released without coordination with peers, and a warning goes to the log.
/// A service handler and a discovery source must not hold a clone of `NetStack`, otherwise the last clone never goes away
/// and only `close()` closes it.
#[derive(Clone)]
pub struct NetStack(pub(super) Arc<NetStackShared>);

/// A weak handle to the stack: it does not keep it alive. Sync upgrades it to `NetStack` for the duration of one `connect`
/// and drops it at once, so the stack's `Drop` still closes everything.
#[derive(Clone)]
pub struct WeakStack(Weak<NetStackShared>);

impl WeakStack {
    /// `None` if the stack has already gone.
    pub fn upgrade(&self) -> Option<NetStack> {
        self.0.upgrade().map(NetStack)
    }
}

#[derive(Clone, Debug)]
pub struct NetSnapshot {
    pub me: PeerId,
    pub transport_names: Vec<&'static str>,
    pub discovery_names: Vec<&'static str>,
    pub known_peer_count: usize,
}

impl NetStack {
    pub fn me(&self) -> PeerId {
        self.0.me
    }

    /// Whether the `closed` flag is raised (set by `close`).
    pub fn is_closed(&self) -> bool {
        self.0.closed.load(Ordering::SeqCst)
    }

    /// Hints from discovery, the first transport in order with `can_reach`; its error goes up.
    pub async fn connect(&self, peer: PeerId, svc: ServiceId) -> NetResult<Arc<dyn PeerConn>> {
        if self.0.closed.load(Ordering::SeqCst) {
            return Err(NetError::Closed);
        }
        if peer == self.0.me {
            return Err(NetError::NoRoute);
        }
        let addr = self
            .0
            .mux
            .lookup(&peer)
            .unwrap_or_else(|| PeerAddr::new(peer));
        self.connect_addr(&addr, svc).await
    }

    /// Connection to a ready address: the first transport in order with `can_reach` gets its hints and
    /// dials; no suitable transport — `NoRoute`, stack closed — `Closed`, own id — `NoRoute`.
    pub async fn connect_addr(
        &self,
        addr: &PeerAddr,
        svc: ServiceId,
    ) -> NetResult<Arc<dyn PeerConn>> {
        if self.0.closed.load(Ordering::SeqCst) {
            return Err(NetError::Closed);
        }
        if addr.id == self.0.me {
            return Err(NetError::NoRoute);
        }
        let transport = self.pick(addr).ok_or(NetError::NoRoute)?;
        transport.add_hints(addr);
        transport.connect(addr, svc).await
    }

    /// Whether there is a transport that will take on dialing this address: the stack is not closed, the address is not its own.
    pub fn can_reach(&self, addr: &PeerAddr) -> bool {
        let open = !self.0.closed.load(Ordering::SeqCst);
        open && addr.id != self.0.me && self.pick(addr).is_some()
    }

    /// A weak handle to this stack.
    pub fn downgrade(&self) -> WeakStack {
        WeakStack(Arc::downgrade(&self.0))
    }

    fn pick(&self, addr: &PeerAddr) -> Option<Arc<dyn Transport>> {
        let mut reach = self.0.transports.iter().filter(|t| t.can_reach(addr));
        reach.next().cloned()
    }

    /// A hint from the application; it reaches the transports through the distribution of findings.
    pub fn add_hints(&self, addr: PeerAddr) {
        if addr.id == self.0.me {
            return;
        }
        self.0.mux.note(MANUAL_SOURCE, addr);
    }

    pub fn set_relays(&self, relays: Vec<RelayRef>) {
        self.0.transports.iter().for_each(|t| t.set_relays(&relays));
    }

    pub fn invite_addrs(&self) -> Vec<PeerAddr> {
        self.0.transports.iter().map(|t| t.local_addr()).collect()
    }

    pub async fn publish(&self) -> NetResult<()> {
        if self.0.closed.load(Ordering::SeqCst) {
            return Err(NetError::Closed);
        }
        self.0.mux.publish(&self.local_merged()).await
    }

    /// Own addresses of all transports, merged in order.
    fn local_merged(&self) -> PeerAddr {
        let mut merged = PeerAddr::new(self.0.me);
        self.0
            .transports
            .iter()
            .for_each(|t| merged.merge(&t.local_addr()));
        merged
    }

    pub fn snapshot(&self) -> NetSnapshot {
        let ts = &self.0.transports;
        NetSnapshot {
            me: self.0.me,
            transport_names: ts.iter().map(|t| t.kind()).collect(),
            discovery_names: self.0.mux.source_names(),
            known_peer_count: self.0.mux.known_peer_count(&self.0.me),
        }
    }

    /// A second and a concurrent call wait for the first and return.
    pub async fn close(&self) {
        let mut done = self.0.close_gate.lock().await;
        if *done {
            return;
        }
        *done = true;
        self.0.closed.store(true, Ordering::SeqCst);
        self.0.mux.stop().await;
        let tasks =
            std::mem::take(&mut *self.0.tasks.lock().unwrap_or_else(PoisonError::into_inner));
        tasks.iter().for_each(|h| h.abort());
        for task in tasks {
            let _ = task.await;
        }
        self.0.handler_tasks.shutdown().await;
        for t in &self.0.transports {
            t.close().await;
        }
    }
}

#[cfg(test)]
#[path = "stack_tests.rs"]
mod tests;
