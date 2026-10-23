//! Ages of the signs of life of every known node, for the `last_seen_sec` of the member rows.
//! It reads the same sources as the peer snapshot (connections, telemetry, the address cache and
//! the document) but not the site checks, and writes nothing.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;

use crate::members::stale_peers::{self, Evidence};
use crate::node::RunningNode;
use crate::transport::net::PeerId;

/// Seconds since the last sign of life of each node that has any; a node without signs is absent.
/// An unreadable document counts as empty.
pub(super) async fn seen_ages(node: &RunningNode, now_unix: u64) -> BTreeMap<PeerId, u64> {
    let doc_seen = stale_peers::read_doc_seen(node.domains.shared_replica().as_ref(), now_unix)
        .await
        .unwrap_or_default();
    let telemetry = node.telemetry.view();
    let members = node.members.ids();
    let no_checks = HashMap::new();
    let ev = Evidence {
        members: &members,
        telemetry: &telemetry,
        now: Instant::now(),
        now_unix,
        check_seen_unix: &no_checks,
        cache_seen_unix: &node.cached_peers,
        doc_seen_unix: &doc_seen,
    };
    known_ids(&ev)
        .into_iter()
        .filter_map(|p| Some((p, stale_peers::age_sec(&ev, p)?)))
        .collect()
}

/// Every node named by at least one source.
fn known_ids(ev: &Evidence<'_>) -> HashSet<PeerId> {
    let mut ids: HashSet<PeerId> = ev.members.iter().copied().collect();
    ids.extend(ev.telemetry.peers.keys().copied());
    ids.extend(ev.cache_seen_unix.keys().copied());
    ids.extend(ev.doc_seen_unix.keys().copied());
    ids
}
