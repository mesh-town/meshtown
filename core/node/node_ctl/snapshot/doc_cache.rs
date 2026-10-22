//! Cache of the snapshot's document parts: the shared document changes rarely, while `collect` runs once a second.
//! The pure `should_read` decides whether to re-read, from the `DocChanges` change counters and the read age.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::net::PeerId;

use crate::checks::CheckView;
use crate::geoip::Country;
use crate::node::RunningNode;
use crate::ports::PortView;

use super::collect::read_doc_parts;

/// Changes made by others to the document are read no more often than this.
pub(super) const REMOTE_MIN_AGE: Duration = Duration::from_secs(10);
/// Safety net: the document is re-read at least this often.
pub(super) const MAX_AGE: Duration = Duration::from_secs(60);

/// Document parts of a collection (without the node's live data).
#[derive(Clone, Default)]
pub(super) struct DocParts {
    pub list: Vec<String>,
    pub views: Vec<CheckView>,
    pub exits: HashMap<PeerId, bool>,
    pub countries: HashMap<PeerId, Country>,
    /// Member versions from the document, without the own one (`collect` adds it on every tick).
    pub program_versions: HashMap<PeerId, String>,
    pub port_views: Vec<PortView>,
    pub doc_seen: HashMap<PeerId, u64>,
}

/// Captured document change counters and the moment of the read.
#[derive(Clone, Copy)]
pub(super) struct Stamp {
    pub local: u64,
    pub remote: u64,
    pub at: Instant,
}

/// Whether to re-read the document: no cache; own change; others' change and ≥ 10 s passed; ≥ 60 s passed.
pub(super) fn should_read(prev: Option<&Stamp>, now: &Stamp) -> bool {
    let Some(p) = prev else { return true };
    let age = now.at.saturating_duration_since(p.at);
    now.local != p.local
        || (now.remote != p.remote && age >= REMOTE_MIN_AGE)
        || age >= MAX_AGE
}

/// Cache of one `attach`: a complete read is stored here, a partial one (`partial`) is not.
#[derive(Default)]
pub(crate) struct DocCache {
    held: Option<(Stamp, DocParts)>,
}

impl DocCache {
    /// Document parts and the `partial` flag; the counters are captured before the document is read.
    pub(super) async fn read_doc_parts_cached(&mut self, node: &RunningNode, now_unix: u64) -> (DocParts, bool) {
        let (local, remote) = node.doc_changes.local_and_remote_counts();
        let now = Stamp { local, remote, at: Instant::now() };
        let prev = self.held.as_ref().map(|(s, _)| s);
        if !should_read(prev, &now) {
            return (self.held.as_ref().map(|(_, p)| p.clone()).unwrap_or_default(), false);
        }
        let (parts, read_complete) = read_doc_parts(node, now_unix).await;
        self.store(now, &parts, read_complete);
        (parts, !read_complete)
    }

    /// Stores a complete read in the cache; a partial one clears the cache — the next tick reads again.
    fn store(&mut self, at: Stamp, parts: &DocParts, read_complete: bool) {
        self.held = read_complete.then(|| (at, parts.clone()));
    }
}

#[cfg(test)]
#[path = "doc_cache_tests.rs"]
mod tests;
