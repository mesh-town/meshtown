//! Facade builder: settings validation, starting discovery and the acceptor of incoming connections.
//! Settings errors are `NetError::Config`, the first one found.

use super::services::{accept_loop, forward_hints, HandlerSet, ServiceHandler, ServiceTable};
use super::stack::{NetStack, NetStackShared};
use crate::transport::net::addr::PeerAddr;
use crate::transport::net::discovery::Discovery;
use crate::transport::net::discovery_mux::DiscoveryMux;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::{NodeSecret, PeerId};
use crate::transport::net::service::ServiceId;
use crate::transport::net::transport::Transport;
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

pub struct NetStackBuilder {
    secret: NodeSecret,
    transports: Vec<Arc<dyn Transport>>,
    discovery: Vec<Arc<dyn Discovery>>,
    discovery_priority: Vec<&'static str>,
    found_tap: Option<mpsc::Sender<PeerAddr>>,
    services: Vec<(ServiceId, Arc<dyn ServiceHandler>)>,
}

impl NetStackBuilder {
    pub fn new(secret: NodeSecret) -> Self {
        NetStackBuilder {
            secret,
            transports: Vec::new(),
            discovery: Vec::new(),
            discovery_priority: Vec::new(),
            found_tap: None,
            services: Vec::new(),
        }
    }

    pub fn transport(mut self, t: Arc<dyn Transport>) -> Self {
        self.transports.push(t);
        self
    }

    pub fn discovery(mut self, d: Arc<dyn Discovery>) -> Self {
        self.discovery.push(d);
        self
    }

    pub fn discovery_priority(mut self, order: Vec<&'static str>) -> Self {
        self.discovery_priority = order;
        self
    }

    /// Tap of every message of the discovery sources (without deduplication); if the receiver is dropped, the source stops.
    pub fn found_tap(mut self, tx: mpsc::Sender<PeerAddr>) -> Self {
        self.found_tap = Some(tx);
        self
    }

    pub fn serve(mut self, svc: ServiceId, handler: Arc<dyn ServiceHandler>) -> Self {
        self.services.push((svc, handler));
        self
    }

    /// The first settings error.
    fn check(&self) -> NetResult<()> {
        let cfg = |msg: &str| Err(NetError::Config(msg.to_string()));
        if tokio::runtime::Handle::try_current().is_err() {
            return cfg("the facade is built inside tokio");
        }
        if self.transports.is_empty() {
            return cfg("no transport is set");
        }
        let me = self.secret.peer_id();
        if self.transports.iter().any(|t| t.me() != me) {
            return cfg("the transport key does not match the node secret");
        }
        let mut seen = HashSet::new();
        if !self.services.iter().all(|(id, _)| seen.insert(*id)) {
            return cfg("a service is specified twice");
        }
        let mut names = HashSet::new();
        if !self.discovery.iter().all(|d| names.insert(d.name())) {
            return cfg("a discovery source is specified twice");
        }
        Ok(())
    }

    pub fn build(self) -> NetResult<NetStack> {
        self.check()?;
        let me = self.secret.peer_id();
        let table = Arc::new(ServiceTable(self.services));
        self.transports.iter().for_each(|t| t.serve(&table.ids()));
        let mut mux = DiscoveryMux::new(self.discovery_priority);
        self.discovery.into_iter().for_each(|d| mux.add(d));
        if let Some(tx) = self.found_tap {
            mux.set_found_tap(tx);
        }
        let mux = Arc::new(mux);
        let hints = mux.subscribe();
        mux.start();
        let handler_tasks = Arc::new(HandlerSet::default());
        let tasks = Self::spawn_stack_tasks(hints, me, &self.transports, &table, &handler_tasks);
        Ok(NetStack(Arc::new(NetStackShared {
            me,
            transports: self.transports,
            mux,
            handler_tasks,
            tasks: Mutex::new(tasks),
            closed: AtomicBool::new(false),
            close_gate: tokio::sync::Mutex::new(false),
        })))
    }

    /// The hint forwarder and one accept loop per transport.
    fn spawn_stack_tasks(
        hints: broadcast::Receiver<PeerAddr>,
        me: PeerId,
        transports: &[Arc<dyn Transport>],
        table: &Arc<ServiceTable>,
        handler_tasks: &Arc<HandlerSet>,
    ) -> Vec<JoinHandle<()>> {
        let forwarder = forward_hints(hints, me, transports.to_vec());
        let mut tasks = vec![crate::transport::net::task_label::spawn(forwarder)];
        for t in transports {
            tasks.push(crate::transport::net::task_label::spawn(accept_loop(
                t.clone(),
                table.clone(),
                handler_tasks.clone(),
            )));
        }
        tasks
    }
}

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
