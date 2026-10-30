//! Reconciliation of two in-memory replicas: both snapshots merged, neighbors introduced, events emitted.

use super::MemReplica;
use crate::transport::net::id::PeerId;
use crate::transport::net::replica::{ReplicaEvent, SyncCount};
use std::sync::Arc;

impl MemReplica {
    /// Exchange of all entries in both directions, getting acquainted and the final events.
    pub(super) fn exchange(&self, other: &Arc<MemReplica>) {
        let (received, sent) = self.merge_snapshots(other);
        self.add_neighbor_pair(other);
        self.emit(synced(other.me, received, sent));
        other.emit(synced(self.me, sent, received));
    }

    /// Both snapshots are taken before either merge; returns what this side received and sent.
    fn merge_snapshots(&self, other: &MemReplica) -> (u64, u64) {
        let (mine, theirs) = (self.snapshot(), other.snapshot());
        let received = self.merge_all(theirs);
        let sent = other.merge_all(mine);
        (received, sent)
    }

    fn add_neighbor_pair(&self, other: &Arc<MemReplica>) {
        self.add_neighbor(other);
        if let Some(me) = self.this.upgrade() {
            other.add_neighbor(&me);
        }
    }
}

fn synced(peer: PeerId, received: u64, sent: u64) -> ReplicaEvent {
    ReplicaEvent::Synced {
        peer,
        outcome: Ok(SyncCount { received, sent }),
    }
}
