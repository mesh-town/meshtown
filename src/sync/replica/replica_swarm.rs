//! Events of the replica's broadcast topic: peers, foreign entries, lag, closing.

use super::entry::check;
use super::replica::{now_micros, MeshReplica};
use super::wire::{decode_push, PROTO_VERSION};
use crate::net::id::PeerId;
use crate::net::replica::ReplicaEvent;
use crate::net::SwarmEvent;
use std::sync::Arc;

/// An entry with `ts_micros` further ahead than this limit is a reason for a log line, but not for refusal.
const FAR_AHEAD_MICROS: u64 = 10 * 60 * 1_000_000;

impl MeshReplica {
    pub(super) fn on_swarm(self: &Arc<Self>, ev: SwarmEvent) {
        match ev {
            SwarmEvent::NeighborUp(peer) => self.neighbor(peer, true),
            SwarmEvent::NeighborDown(peer) => self.neighbor(peer, false),
            SwarmEvent::Received { from, msg } => self.received(from, &msg),
            SwarmEvent::Lagged => self.resync_all(),
            SwarmEvent::Closed(text) => tracing::warn!("[sync] replica topic {} closed: {text}", self.id.fmt_short()),
        }
    }

    /// A peer came or left; only the node with the smaller `PeerId` dials.
    fn neighbor(self: &Arc<Self>, peer: PeerId, up: bool) {
        self.set_neighbor(peer, up);
        self.emit(ReplicaEvent::Neighbor { peer, up });
        if up && self.wiring.me.as_bytes() < peer.as_bytes() {
            self.spawn_dial(peer);
        }
    }

    /// Broadcast packets were missed: reconciliation with all current peers.
    fn resync_all(self: &Arc<Self>) {
        for peer in self.neighbor_list() {
            self.spawn_dial(peer);
        }
    }

    /// Periodic reconciliation: only towards neighbors with a larger `PeerId`, so each pair is reconciled by one side.
    pub(super) fn resync_smaller(self: &Arc<Self>) {
        let me = self.wiring.me;
        for peer in self.neighbor_list().into_iter().filter(|p| me.as_bytes() < p.as_bytes()) {
            self.spawn_dial(peer);
        }
    }

    fn spawn_dial(self: &Arc<Self>, peer: PeerId) {
        let Some(slot) = self.claim(peer) else { return };
        let owner = self.clone();
        tokio::spawn(async move { owner.dial_session(peer, slot).await });
    }

    /// A foreign live broadcast: signature check, merge, `RemoteChange`, deferred save.
    fn received(&self, from: PeerId, msg: &[u8]) {
        let Ok(push) = decode_push(msg) else {
            return tracing::warn!("[sync] corrupt broadcast from {from}");
        };
        if push.version != PROTO_VERSION || push.replica != self.id.0 {
            return;
        }
        let (mut changed, mut rejected) = (false, 0u32);
        let far = now_micros().saturating_add(FAR_AHEAD_MICROS);
        let mut state = self.lock();
        for w in push.entries {
            let Ok(author) = check(&self.id, &w) else {
                rejected += 1;
                continue;
            };
            if w.ts_micros > far && self.first_warning(author) {
                tracing::warn!("[sync] author {author}'s clock is far ahead (entry {} µs)", w.ts_micros);
            }
            changed |= state.merge(w);
        }
        drop(state);
        if rejected > 0 {
            tracing::warn!("[sync] dropped {rejected} entries from {from}: bad signature or limit");
        }
        if changed {
            self.emit(ReplicaEvent::RemoteChange);
            self.saver.save_debounced(self.state.clone());
        }
    }
}

// Paused-clock tests need tokio test-util, which only the node layer enables.
#[cfg(all(test, feature = "net-hyper"))]
#[path = "replica_resync_tests.rs"]
mod resync_tests;
