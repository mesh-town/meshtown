//! `mem` sync: an in-memory network node — a broadcast topic and the named replicas "net" and "domains".

use super::replica::MemReplica;
use super::swarm::MemSwarm;
use super::sync_hub::MemSyncNet;
use crate::net::error::{NetError, NetResult};
use crate::net::id::PeerId;
use crate::net::replica::Replica;
use crate::net::stream::BoxFuture;
use crate::net::sync::Membership;
use crate::net::sync_backend::{Swarm, SyncBackend, REPLICA_DOMAINS, REPLICA_NET};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct MemSync {
    hub: Arc<MemSyncNet>,
    seed: [u8; 32],
    me: PeerId,
    replicas: Mutex<HashMap<&'static str, Arc<MemReplica>>>,
}

impl MemSync {
    pub(super) fn new(hub: Arc<MemSyncNet>, seed: [u8; 32], me: PeerId) -> Arc<MemSync> {
        Arc::new(MemSync {
            hub,
            seed,
            me,
            replicas: Mutex::default(),
        })
    }

    /// A replica by name: created on first access, then the same one.
    pub fn replica_mem(&self, name: &str) -> NetResult<Arc<MemReplica>> {
        let name = match name {
            REPLICA_NET => REPLICA_NET,
            REPLICA_DOMAINS => REPLICA_DOMAINS,
            other => return Err(NetError::Config(format!("no replica \"{other}\""))),
        };
        let mut map = self.replicas.lock().unwrap_or_else(|e| e.into_inner());
        let made = map.entry(name).or_insert_with(|| self.create(name));
        Ok(made.clone())
    }

    fn create(&self, name: &'static str) -> Arc<MemReplica> {
        let made = MemReplica::new(name, self.seed, self.me, Arc::downgrade(&self.hub));
        self.hub.register_replica(&made, (name, self.seed, self.me));
        made
    }

    fn join(&self, topic: [u8; 32], bootstrap: &[PeerId]) -> Swarm {
        let (swarm, events) = MemSwarm::new(self.me, topic, Arc::downgrade(&self.hub));
        self.hub.register_swarm(topic, &swarm);
        swarm.join_now(bootstrap);
        Swarm {
            membership: swarm as Arc<dyn Membership>,
            events,
            pump: None,
        }
    }
}

impl SyncBackend for MemSync {
    fn name(&self) -> &'static str {
        "mem"
    }

    fn join_swarm(&self, topic: [u8; 32], bootstrap: &[PeerId]) -> BoxFuture<'_, NetResult<Swarm>> {
        let bootstrap = bootstrap.to_vec();
        Box::pin(async move { Ok(self.join(topic, &bootstrap)) })
    }

    fn open_replica(&self, name: &str) -> BoxFuture<'_, NetResult<Arc<dyn Replica>>> {
        let found = self.replica_mem(name);
        Box::pin(async move { Ok(found? as Arc<dyn Replica>) })
    }
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
