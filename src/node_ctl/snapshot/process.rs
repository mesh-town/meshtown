//! Parts of the snapshot that do not read the node: the status from the slot phase and the overlay of process fields.

use super::{link_status, Collected};
use crate::node_ctl::state::Phase;
use crate::web::{LogSummary, NodeSnapshot, NodeStatus};
use crate::{output, telemetry};

/// How many of the last log lines go into the snapshot.
const LOG_TAIL: usize = 6;

/// Process fields of the snapshot (later — the process block of `/api/state`); `boot_id`/`uptime_sec` are not part of it — they belong to the slot.
pub(in crate::node_ctl) struct ProcessFacts {
    pub uptime_secs: f64,
    pub rss_kib: u64,
    pub rss_peak_kib: u64,
    pub log: LogSummary,
}

impl ProcessFacts {
    /// Takes them from the process: `output::uptime_secs()`, `telemetry::rss_kib()`, `output::log_summary(LOG_TAIL)`.
    pub(in crate::node_ctl) fn collect() -> Self {
        let (rss_kib, rss_peak_kib) = telemetry::rss_kib();
        ProcessFacts {
            uptime_secs: output::uptime_secs(),
            rss_kib,
            rss_peak_kib,
            log: output::log_summary(LOG_TAIL),
        }
    }
}

/// NoNetwork → Offline, Starting → Starting, Stopping → Stopping, Up without a cache → Starting, Up with a cache → `link_status`.
pub(in crate::node_ctl) fn status_of(phase: &Phase, cache: Option<&Collected>) -> NodeStatus {
    match phase {
        Phase::NoNetwork => NodeStatus::Offline,
        Phase::Starting => NodeStatus::Starting,
        Phase::Stopping => NodeStatus::Stopping,
        Phase::Up => cache.map_or(NodeStatus::Starting, link_status),
    }
}

/// Sets `now`, `node.rss_kib`, `node.rss_peak_kib`, `log`; leaves other fields alone.
pub(in crate::node_ctl) fn overlay_process(snap: &mut NodeSnapshot, facts: ProcessFacts) {
    snap.now = facts.uptime_secs;
    snap.node.rss_kib = facts.rss_kib;
    snap.node.rss_peak_kib = facts.rss_peak_kib;
    snap.log = facts.log;
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
