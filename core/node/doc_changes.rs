//! Counters of changes to the network's shared document: `log_events` increments them on replica
//! events, and the snapshot cache (`node_ctl/snapshot/doc_cache.rs`) uses them to decide whether to re-read the document.

use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;

/// How many times the document was changed by us (`local`) and by others or their content (`remote`).
#[derive(Debug, Default)]
pub struct DocChanges {
    local: AtomicU64,
    remote: AtomicU64,
}

impl DocChanges {
    /// Our own write to the document (event `ReplicaEvent::LocalChange`).
    pub fn bump_local(&self) {
        self.local.fetch_add(1, Ordering::Relaxed);
    }

    /// Someone else's write or its content (event `ReplicaEvent::RemoteChange`).
    pub fn bump_remote(&self) {
        self.remote.fetch_add(1, Ordering::Relaxed);
    }

    /// Current values of `(local, remote)`.
    pub fn local_and_remote_counts(&self) -> (u64, u64) {
        (
            self.local.load(Ordering::Relaxed),
            self.remote.load(Ordering::Relaxed),
        )
    }
}
