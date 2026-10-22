//! Membership of a `mem` topic: the nodes of one topic of the hub connect pairwise on an invite from either side.

use super::sync_hub::MemSyncNet;
use crate::net::error::NetResult;
use crate::net::id::PeerId;
use crate::net::stream::BoxFuture;
use crate::net::sync::{Membership, SwarmEvent};
use bytes::Bytes;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use tokio::sync::broadcast;

const EVENT_QUEUE: usize = 256;

#[derive(Default)]
struct SwarmState {
    neighbors: Vec<(PeerId, Weak<MemSwarm>)>,
    invited_not_linked: Vec<PeerId>,
}

pub struct MemSwarm {
    me: PeerId,
    topic: [u8; 32],
    hub: Weak<MemSyncNet>,
    this: Weak<MemSwarm>,
    state: Mutex<SwarmState>,
    tx: broadcast::Sender<SwarmEvent>,
}

impl MemSwarm {
    /// A new node of the topic and the first receiver of its events (created before linking).
    pub(super) fn new(
        me: PeerId,
        topic: [u8; 32],
        hub: Weak<MemSyncNet>,
    ) -> (Arc<MemSwarm>, broadcast::Receiver<SwarmEvent>) {
        let (tx, rx) = broadcast::channel(EVENT_QUEUE);
        let swarm = Arc::new_cyclic(|this| MemSwarm {
            me,
            topic,
            hub,
            this: this.clone(),
            state: Mutex::default(),
            tx,
        });
        (swarm, rx)
    }

    fn lock(&self) -> MutexGuard<'_, SwarmState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn is_invited(&self, peer: PeerId) -> bool {
        self.lock().invited_not_linked.contains(&peer)
    }

    fn is_neighbor(&self, peer: PeerId) -> bool {
        self.lock().neighbors.iter().any(|n| n.0 == peer)
    }

    /// Remembers a neighbor and reports `NeighborUp`; a repeat is silent.
    fn add_neighbor(&self, other: &Arc<MemSwarm>) {
        let mut state = self.lock();
        let known = state.neighbors.iter().any(|n| n.0 == other.me);
        if !known {
            state.neighbors.push((other.me, Arc::downgrade(other)));
        }
        state.invited_not_linked.retain(|p| *p != other.me);
        drop(state);
        if !known {
            let _ = self.tx.send(SwarmEvent::NeighborUp(other.me));
        }
    }

    fn should_link(&self, other: &MemSwarm) -> bool {
        let wanted = self.is_invited(other.me) || other.is_invited(self.me);
        other.me != self.me && wanted && !self.is_neighbor(other.me)
    }

    /// Links with everyone who is already in the topic and invited from one of the sides.
    fn link_all(&self) {
        let Some(me) = self.this.upgrade() else {
            return;
        };
        let members = self.hub.upgrade().map(|h| h.members(self.topic));
        for other in members.unwrap_or_default() {
            if self.should_link(&other) {
                self.add_neighbor(&other);
                other.add_neighbor(&me);
            }
        }
    }

    pub(super) fn join_now(&self, peers: &[PeerId]) {
        let mut state = self.lock();
        let new = peers.iter().filter(|p| **p != self.me);
        let fresh: Vec<PeerId> = new
            .filter(|p| !state.invited_not_linked.contains(p))
            .copied()
            .collect();
        state.invited_not_linked.extend(fresh);
        drop(state);
        self.link_all();
    }

    fn broadcast_now(&self, msg: Bytes) {
        let neighbors: Vec<_> = self
            .lock()
            .neighbors
            .iter()
            .filter_map(|n| n.1.upgrade())
            .collect();
        for n in neighbors {
            let _ = n.tx.send(SwarmEvent::Received {
                from: self.me,
                msg: msg.clone(),
            });
        }
    }
}

impl Membership for MemSwarm {
    fn join(&self, peers: &[PeerId]) -> BoxFuture<'_, NetResult<()>> {
        let peers = peers.to_vec();
        Box::pin(async move {
            self.join_now(&peers);
            Ok(())
        })
    }

    fn broadcast(&self, msg: Bytes) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async move {
            self.broadcast_now(msg);
            Ok(())
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<SwarmEvent> {
        self.tx.subscribe()
    }
}
