//! Service handlers: the table, accepting incoming connections, handing out hints to the transports.
//! An incoming connection to an unserved service is closed before the handler.

use crate::transport::net::addr::PeerAddr;
use crate::transport::net::id::PeerId;
use crate::transport::net::service::ServiceId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::transport::{PeerConn, Transport};
use std::sync::{Arc, Mutex, PoisonError};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::task::JoinSet;

/// Close code for an incoming connection to an unserved service.
pub(super) const UNSERVED_CODE: u32 = 1;

pub trait ServiceHandler: Send + Sync {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()>;
}

pub(super) struct ServiceTable(pub(super) Vec<(ServiceId, Arc<dyn ServiceHandler>)>);

impl ServiceTable {
    pub(super) fn get(&self, svc: ServiceId) -> Option<Arc<dyn ServiceHandler>> {
        let found = self.0.iter().find(|(id, _)| *id == svc);
        found.map(|(_, h)| h.clone())
    }

    pub(super) fn ids(&self) -> Vec<ServiceId> {
        self.0.iter().map(|(id, _)| *id).collect()
    }
}

/// Handler tasks: a panic stays inside its own task, on close all of them are cancelled.
#[derive(Default)]
pub(super) struct HandlerSet(Mutex<JoinSet<()>>);

impl HandlerSet {
    /// First collects the finished results, otherwise they pile up over the whole life of the node.
    pub(super) fn spawn(&self, fut: BoxFuture<'static, ()>) {
        let mut set = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        while set.try_join_next().is_some() {}
        crate::transport::net::task_label::spawn_in(&mut set, fut);
    }

    /// Synchronously aborts all tasks of the set without waiting for them (the `Drop` safety net).
    pub(super) fn abort_all(&self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .abort_all();
    }

    pub(super) async fn shutdown(&self) {
        let mut set = std::mem::take(&mut *self.0.lock().unwrap_or_else(PoisonError::into_inner));
        set.abort_all();
        while set.join_next().await.is_some() {}
    }
}

/// Accepts incoming connections of the transport until `None`, without restarting.
pub(super) async fn accept_loop(
    t: Arc<dyn Transport>,
    table: Arc<ServiceTable>,
    handler_tasks: Arc<HandlerSet>,
) {
    while let Some((svc, conn)) = t.accept().await {
        dispatch(&table, &handler_tasks, svc, conn);
    }
}

fn dispatch(
    table: &ServiceTable,
    handler_tasks: &HandlerSet,
    svc: ServiceId,
    conn: Arc<dyn PeerConn>,
) {
    let Some(handler) = table.get(svc) else {
        tracing::warn!(peer = %conn.peer(), "incoming connection to an unserved service closed");
        conn.close(UNSERVED_CODE, b"unserved");
        return;
    };
    handler_tasks.spawn(handler.handle(conn));
}

/// Hands out discovery findings to all transports except hints about itself.
pub(super) async fn forward_hints(
    mut rx: broadcast::Receiver<PeerAddr>,
    me: PeerId,
    ts: Vec<Arc<dyn Transport>>,
) {
    loop {
        match rx.recv().await {
            Ok(addr) if addr.id != me => ts.iter().for_each(|t| t.add_hints(&addr)),
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}
