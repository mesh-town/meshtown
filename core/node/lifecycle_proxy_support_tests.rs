//! Helpers for the process-proxy cycle tests: ports, settings and a replica that holds the shutdown flush.

use super::*;

pub(super) fn exit_mark(mark: u8) -> crate::transport::net::PeerId {
    NodeSecret::from_bytes(&[mark; 32]).peer_id()
}

pub(super) fn process_on(port: u16) -> crate::config::config_file::ProcessSettings {
    crate::config::config_file::ProcessSettings {
        socks_port: port,
        ..crate::config::config_file::ProcessSettings::default()
    }
}

pub(super) fn settings_on(port: u16) -> Settings {
    Settings {
        process: process_on(port),
        ..Settings::default()
    }
}

/// A free port on loopback: bind with port zero, remember it, close.
pub(super) fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

pub(super) fn can_bind(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Makes the node's shutdown wait at its last step (saving the replicas) until the returned gate is closed:
/// the test then looks at the state of a shutdown that is certainly still running.
pub(in crate::node::lifecycle) async fn hold_replica_flush(
    node: &mut RunningNode,
) -> Arc<tokio::sync::Semaphore> {
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let held = |replica: Arc<dyn crate::transport::net::Replica>| -> Arc<dyn crate::transport::net::Replica> {
        Arc::new(HeldFlush {
            inner: replica,
            gate: gate.clone(),
        })
    };
    let shared = held(node.domains.shared_replica());
    let own = held(node.domains.own_replica());
    node.domains = crate::proxy::domains::DomainList::open(shared, own)
        .await
        .expect("a list over the held replicas");
    gate
}

/// A replica that does everything its inner one does, except `flush`, which waits for the gate to close.
pub(super) struct HeldFlush {
    inner: Arc<dyn crate::transport::net::Replica>,
    gate: Arc<tokio::sync::Semaphore>,
}

impl crate::transport::net::Replica for HeldFlush {
    fn id(&self) -> crate::transport::net::ReplicaId {
        self.inner.id()
    }

    fn me(&self) -> crate::transport::net::PeerId {
        self.inner.me()
    }

    fn set_own(
        &self,
        key: &[u8],
        value: bytes::Bytes,
    ) -> crate::transport::net::BoxFuture<'_, crate::transport::net::NetResult<()>> {
        self.inner.set_own(key, value)
    }

    fn del_own(
        &self,
        key: &[u8],
    ) -> crate::transport::net::BoxFuture<'_, crate::transport::net::NetResult<()>> {
        self.inner.del_own(key)
    }

    fn get(
        &self,
        author: crate::transport::net::PeerId,
        key: &[u8],
    ) -> crate::transport::net::BoxFuture<
        '_,
        crate::transport::net::NetResult<Option<crate::transport::net::ReplicaEntry>>,
    > {
        self.inner.get(author, key)
    }

    fn scan(
        &self,
        prefix: &[u8],
    ) -> crate::transport::net::BoxFuture<
        '_,
        crate::transport::net::NetResult<Vec<crate::transport::net::ReplicaEntry>>,
    > {
        self.inner.scan(prefix)
    }

    fn latest_per_key(
        &self,
        prefix: &[u8],
    ) -> crate::transport::net::BoxFuture<
        '_,
        crate::transport::net::NetResult<Vec<crate::transport::net::ReplicaEntry>>,
    > {
        self.inner.latest_per_key(prefix)
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<crate::transport::net::ReplicaEvent> {
        self.inner.subscribe()
    }

    /// The inner replica already syncs; the list built over this wrapper must not start it twice.
    fn start_sync(
        &self,
    ) -> crate::transport::net::BoxFuture<'_, crate::transport::net::NetResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn sync_with(
        &self,
        peer: crate::transport::net::PeerId,
    ) -> crate::transport::net::BoxFuture<'_, crate::transport::net::NetResult<()>> {
        self.inner.sync_with(peer)
    }

    fn flush(&self) -> crate::transport::net::BoxFuture<'_, crate::transport::net::NetResult<()>> {
        Box::pin(async move {
            let _ = self.gate.acquire().await;
            self.inner.flush().await
        })
    }
}
