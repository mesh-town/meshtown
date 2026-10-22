//! Sync hub for `mem`: replicas keyed by (name, network, node) and nodes by topic as `[u8; 32]`; all references are weak.

use super::replica::MemReplica;
use super::swarm::MemSwarm;
use super::sync::MemSync;
use crate::net::id::PeerId;
use crate::net::sync_backend::{SyncBackend, SyncCtor, SyncCtx};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

type ReplicaKey = (&'static str, [u8; 32], PeerId);

#[derive(Default)]
struct SyncNetState {
    replicas: HashMap<ReplicaKey, Weak<MemReplica>>,
    topics: HashMap<[u8; 32], Vec<Weak<MemSwarm>>>,
}

/// Shared sync "network": nodes created from one hub see each other.
#[derive(Default)]
pub struct MemSyncNet {
    inner: Mutex<SyncNetState>,
}

impl MemSyncNet {
    pub fn new() -> Arc<MemSyncNet> {
        Arc::new(MemSyncNet::default())
    }

    /// Node of network `ctx.network_seed` with author `ctx.secret.peer_id()`.
    pub fn backend(self: &Arc<Self>, ctx: SyncCtx) -> Arc<MemSync> {
        MemSync::new(self.clone(), ctx.network_seed, ctx.secret.peer_id())
    }

    fn lock(&self) -> MutexGuard<'_, SyncNetState> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn register_replica(&self, r: &Arc<MemReplica>, key: ReplicaKey) {
        self.lock().replicas.insert(key, Arc::downgrade(r));
    }

    pub(super) fn replica(
        &self,
        name: &'static str,
        seed: [u8; 32],
        peer: PeerId,
    ) -> Option<Arc<MemReplica>> {
        self.lock().replicas.get(&(name, seed, peer))?.upgrade()
    }

    /// A node joins a topic; dead entries of the topic are removed.
    pub(super) fn register_swarm(&self, topic: [u8; 32], swarm: &Arc<MemSwarm>) {
        let mut inner = self.lock();
        let list = inner.topics.entry(topic).or_default();
        list.retain(|w| w.strong_count() > 0);
        list.push(Arc::downgrade(swarm));
    }

    pub(super) fn members(&self, topic: [u8; 32]) -> Vec<Arc<MemSwarm>> {
        let inner = self.lock();
        let list = inner.topics.get(&topic);
        list.map(|l| l.iter().filter_map(Weak::upgrade).collect())
            .unwrap_or_default()
    }
}

/// Constructor for `Registry::add_sync("mem", …)`.
pub fn sync_ctor(net: Arc<MemSyncNet>) -> SyncCtor {
    Box::new(move |ctx| {
        let backend = net.backend(ctx);
        Box::pin(async move { Ok(backend as Arc<dyn SyncBackend>) })
    })
}
