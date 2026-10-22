//! The age of a member by all signs of life and the "gone" rule (7 days without news).
//!
//! What it does: counts how many seconds ago a member gave a sign of life (the minimum over
//! membership, telemetry, the freshest check in the document, an entry in the address cache and the freshest
//! entry of any document key); decides whether it is "gone" (`GONE_AFTER_SECS`); reads from
//! the document the time of each author's last write.
//! What it doesn't do: deletes and writes nothing — the snapshot (`node_ctl::snapshot::peers`) and
//! the hourly sweep (`stale_prune`) do the cutting; doesn't read the document's blobs.
//! Links: `node_ctl::snapshot::{peers, collect}` build `Evidence` and call `age_sec`,
//! `read_doc_seen`; `stale_prune::prune_from` calls `is_gone`.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::net::{ReplicaEntry, NetError, PeerId, Replica};
use anyhow::Result;

use crate::checks::CheckView;
use crate::telemetry::TelemetryView;

/// A member with no news for longer than this period is "gone": 7 days.
pub const GONE_AFTER_SECS: u64 = 7 * 24 * 3600;

/// All signs of life of the members, collected once per snapshot or tick.
pub struct Evidence<'a> {
    /// Members connected right now.
    pub members: &'a HashSet<PeerId>,
    /// Telemetry of this run (`last_seen` is a monotonic clock).
    pub telemetry: &'a TelemetryView,
    /// The monotonic "now" for telemetry.
    pub now: Instant,
    /// The Unix "now".
    pub now_unix: u64,
    /// The time of the author's freshest check.
    pub check_seen_unix: &'a HashMap<PeerId, u64>,
    /// The address cache on disk (`seen_unix`).
    pub cache_seen_unix: &'a HashMap<PeerId, u64>,
    /// The time of the author's last write in the document.
    pub doc_seen_unix: &'a HashMap<PeerId, u64>,
}

/// The age of a sign of life, `None` — no signs at all. A network member — 0; otherwise the minimum of
/// telemetry and the three maps; a future time gives 0.
pub fn age_sec(ev: &Evidence<'_>, id: PeerId) -> Option<u64> {
    if ev.members.contains(&id) {
        return Some(0);
    }
    let live = ev
        .telemetry
        .peers
        .get(&id)
        .map(|p| ev.now.saturating_duration_since(p.last_seen).as_secs());
    let ages = [ev.check_seen_unix, ev.cache_seen_unix, ev.doc_seen_unix]
        .into_iter()
        .filter_map(|m| m.get(&id).map(|t| ev.now_unix.saturating_sub(*t)));
    live.into_iter().chain(ages).min()
}

/// Whether a member is gone: the age is strictly greater than `GONE_AFTER_SECS` or there are no signs at all.
pub fn is_gone(age: Option<u64>) -> bool {
    age.is_none_or(|a| a > GONE_AFTER_SECS)
}

/// The time of the freshest check entry of each author, only over the domains in `domain_names`; an entry without
/// parsed content is skipped (as in `author_stats`).
pub fn latest_check_unix_by_author(views: &[CheckView], domain_names: &[String]) -> HashMap<PeerId, u64> {
    let mut out: HashMap<PeerId, u64> = HashMap::new();
    for v in views {
        let Some(r) = &v.record else { continue };
        if !domain_names.iter().any(|d| d == &v.domain) {
            continue;
        }
        let e = out.entry(v.author).or_insert(0);
        *e = (*e).max(r.at);
    }
    out
}

/// Folds entries into seconds: per author the greatest stamp `ts_micros / 1_000_000`, a stamp from
/// the future is clamped to `now_unix`.
pub fn fold_doc_seen(entries: &[ReplicaEntry], now_unix: u64) -> HashMap<PeerId, u64> {
    let mut out: HashMap<PeerId, u64> = HashMap::new();
    for e in entries {
        let secs = (e.ts_micros / 1_000_000).min(now_unix);
        let seen = out.entry(e.author).or_insert(0);
        *seen = (*seen).max(secs);
    }
    out
}

/// Reads the stamps of all entries (tombstones included) from the replica and folds them with `fold_doc_seen`.
pub async fn read_doc_seen(r: &dyn Replica, now_unix: u64) -> Result<HashMap<PeerId, u64>> {
    let entries = r.scan(b"").await.map_err(NetError::into_anyhow)?;
    Ok(fold_doc_seen(&entries, now_unix))
}

#[cfg(test)]
#[path = "stale_peers_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "stale_peers_doc_tests.rs"]
mod doc_tests;
