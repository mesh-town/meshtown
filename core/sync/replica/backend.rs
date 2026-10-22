//! The "mesh" sync: membership by flooding (`sync/flood`) and its own replica (`MeshReplica`).

use super::host::ReplicaHost;
use super::import::LegacyRead;
use super::service::ReplicaService;
use crate::net::error::{NetError, NetResult};
use crate::net::id::PeerId;
use crate::net::replica::Replica;
use crate::net::stream::BoxFuture;
use crate::net::sync_backend::{Swarm, SyncCtor, SyncCtx};
use crate::net::{ServiceHandler, ServiceId, SyncBackend, WeakStack, REPLICA};
use crate::sync::flood::{FloodSync, Tuning};
use std::sync::Arc;

pub use crate::sync::flood::NAME;

/// The flooding delegate: it has no replicas, `MeshSync` holds them.
pub(super) struct NoReplicas;

impl SyncBackend for NoReplicas {
    fn name(&self) -> &'static str {
        NAME
    }

    fn join_swarm(&self, _topic: [u8; 32], _bootstrap: &[PeerId]) -> BoxFuture<'_, NetResult<Swarm>> {
        Box::pin(async { Err(NetError::Config("topics are opened by MeshSync".into())) })
    }

    fn open_replica(&self, _name: &str) -> BoxFuture<'_, NetResult<Arc<dyn Replica>>> {
        Box::pin(async { Err(NetError::Config("replicas are opened by MeshSync".into())) })
    }
}

pub struct MeshSync {
    pub(super) flood: Arc<FloodSync>,
    pub(super) host: Arc<ReplicaHost>,
}

impl MeshSync {
    /// Builds a sync over a ready replica host; the replicas hold only a `Weak` to flooding.
    pub(super) fn assemble(ctx: &SyncCtx, host: Arc<ReplicaHost>, tuning: Tuning) -> Arc<MeshSync> {
        let flood = FloodSync::new(ctx, Arc::new(NoReplicas), tuning);
        host.attach_flood(Arc::downgrade(&flood));
        Arc::new(MeshSync { flood, host })
    }
}

impl SyncBackend for MeshSync {
    fn name(&self) -> &'static str {
        NAME
    }

    fn join_swarm(&self, topic: [u8; 32], bootstrap: &[PeerId]) -> BoxFuture<'_, NetResult<Swarm>> {
        self.flood.join_swarm(topic, bootstrap)
    }

    fn open_replica(&self, name: &str) -> BoxFuture<'_, NetResult<Arc<dyn Replica>>> {
        let found = self.host.open(name);
        Box::pin(async move { Ok(found? as Arc<dyn Replica>) })
    }

    fn service(&self) -> Option<(ServiceId, Arc<dyn ServiceHandler>)> {
        self.flood.service()
    }

    fn services(&self) -> Vec<(ServiceId, Arc<dyn ServiceHandler>)> {
        let mut all = self.flood.services();
        all.push((REPLICA, Arc::new(ReplicaService::new(self.host.clone()))));
        all
    }

    fn attach_stack(&self, stack: WeakStack) {
        self.host.attach_stack(stack.clone());
        self.flood.attach_stack(stack);
    }
}

/// The constructor for `Registry::add_sync("mesh", …)`: the files and the `docs-store/` import happen before the sync is returned.
pub fn sync_ctor(legacy: Option<LegacyRead>) -> SyncCtor {
    Box::new(move |ctx| {
        let legacy = legacy.clone();
        Box::pin(async move {
            let host = ReplicaHost::load(&ctx, legacy).await?;
            Ok(MeshSync::assemble(&ctx, host, Tuning::default()) as Arc<dyn SyncBackend>)
        })
    })
}
