//! `SplitTransport`: a transport made of a "connector + carrier" pair.
//! The connector finds a path, the carrier builds a data stream on it; there is no protocol of its own here.

use crate::transport::net::addr::{PeerAddr, RelayRef};
use crate::transport::net::carrier::{Carrier, CarrierRole};
use crate::transport::net::connect::{Connector, PacketPath};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::link::NetInfo;
use crate::transport::net::service::ServiceId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::transport::{PeerConn, Transport};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::mpsc;
use tokio::task::{JoinHandle, JoinSet};

/// Queue of accepted connections; when full, the path task waits, not the accept loop.
const ACCEPT_QUEUE: usize = 64;

type Incoming = (ServiceId, Arc<dyn PeerConn>);

pub struct SplitTransport {
    me: PeerId,
    connector: Arc<dyn Connector>,
    carrier: Arc<dyn Carrier>,
    served_service_ids: Arc<RwLock<Arc<[ServiceId]>>>,
    incoming: tokio::sync::Mutex<mpsc::Receiver<Incoming>>,
    closed: AtomicBool,
    task: Mutex<Option<JoinHandle<()>>>,
}

/// Starts the carrier on an incoming path; an error is silent (the path is closed by the carrier).
async fn accept_one(
    carrier: Arc<dyn Carrier>,
    path: Box<dyn PacketPath>,
    served_service_ids: Arc<[ServiceId]>,
    out: mpsc::Sender<Incoming>,
) {
    let Ok(done) = carrier
        .start(path, CarrierRole::Accept(served_service_ids))
        .await
    else {
        return;
    };
    let _ = out.send(done).await;
}

/// Accept loop: one task per path; `listen` → `None` resets all tasks and closes the queue.
async fn accept_loop(
    connector: Arc<dyn Connector>,
    carrier: Arc<dyn Carrier>,
    served_service_ids: Arc<RwLock<Arc<[ServiceId]>>>,
    out: mpsc::Sender<Incoming>,
) {
    let mut set = JoinSet::new();
    while let Some(path) = connector.listen().await {
        while set.try_join_next().is_some() {}
        let served_service_ids = served_service_ids
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        set.spawn(accept_one(
            carrier.clone(),
            path,
            served_service_ids,
            out.clone(),
        ));
    }
}

impl SplitTransport {
    /// Requires a tokio runtime: spawns the accept loop.
    pub fn new(me: PeerId, connector: Arc<dyn Connector>, carrier: Arc<dyn Carrier>) -> Self {
        debug_assert!(me == connector.local_addr().id);
        let served_service_ids: Arc<RwLock<Arc<[ServiceId]>>> =
            Arc::new(RwLock::new(Arc::from([])));
        let (out, rx) = mpsc::channel(ACCEPT_QUEUE);
        let task = tokio::spawn(accept_loop(
            connector.clone(),
            carrier.clone(),
            served_service_ids.clone(),
            out,
        ));
        SplitTransport {
            me,
            connector,
            carrier,
            served_service_ids,
            incoming: tokio::sync::Mutex::new(rx),
            closed: AtomicBool::new(false),
            task: Mutex::new(Some(task)),
        }
    }

    async fn connect_inner(&self, to: PeerAddr, svc: ServiceId) -> NetResult<Arc<dyn PeerConn>> {
        if self.closed.load(Ordering::Acquire) {
            return Err(NetError::Closed);
        }
        let path = self.connector.dial(&to).await?;
        if !self.carrier.can_carry(&path.info()) {
            path.close();
            return Err(NetError::NoRoute);
        }
        let (_, conn) = self.carrier.start(path, CarrierRole::Dial(svc)).await?;
        Ok(conn)
    }

    async fn close_inner(&self) {
        self.closed.store(true, Ordering::Release);
        self.connector.close().await;
        let task = self.task.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}

impl Drop for SplitTransport {
    fn drop(&mut self) {
        let task = self.task.get_mut().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = task.take() {
            task.abort();
        }
    }
}

impl Transport for SplitTransport {
    fn kind(&self) -> &'static str {
        self.connector.kind()
    }

    fn me(&self) -> PeerId {
        self.me
    }

    fn local_addr(&self) -> PeerAddr {
        self.connector.local_addr()
    }

    fn can_reach(&self, to: &PeerAddr) -> bool {
        self.connector.can_reach(to)
    }

    fn add_hints(&self, addr: &PeerAddr) {
        self.connector.add_hints(addr);
    }

    fn set_relays(&self, relays: &[RelayRef]) {
        self.connector.set_relays(relays);
    }

    fn serve(&self, svcs: &[ServiceId]) {
        let mut cur = self
            .served_service_ids
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *cur = Arc::from(svcs);
    }

    fn connect(
        &self,
        to: &PeerAddr,
        svc: ServiceId,
    ) -> BoxFuture<'_, NetResult<Arc<dyn PeerConn>>> {
        Box::pin(self.connect_inner(to.clone(), svc))
    }

    fn accept(&self) -> BoxFuture<'_, Option<Incoming>> {
        Box::pin(async move { self.incoming.lock().await.recv().await })
    }

    fn net_info(&self) -> NetInfo {
        self.connector.net_info()
    }

    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(self.close_inner())
    }
}

#[cfg(test)]
#[path = "split_tests.rs"]
mod tests;
