//! Parts of the snapshot that do not read the node: the status from the slot phase and the overlay of process fields.

use crate::node::node_ctl::state::Phase;
use crate::web_gui::{LogSummary, NodeSnapshot, NodeStatus};
use crate::{node::output, node::telemetry};

/// How many of the last log lines go into the snapshot.
const LOG_TAIL: usize = 6;

/// Process fields of the snapshot (later — the process block of `/api/state`); `boot_id`/`uptime_sec` are not part of it — they belong to the slot.
pub(in crate::node::node_ctl) struct ProcessFacts {
    pub uptime_secs: f64,
    pub rss_kib: u64,
    pub rss_peak_kib: u64,
    pub log: LogSummary,
}

impl ProcessFacts {
    /// Takes them from the process: `output::uptime_secs()`, `telemetry::rss_kib()`, `output::log_summary(LOG_TAIL)`.
    pub(in crate::node::node_ctl) fn collect() -> Self {
        let (rss_kib, rss_peak_kib) = telemetry::rss_kib();
        ProcessFacts {
            uptime_secs: output::uptime_secs(),
            rss_kib,
            rss_peak_kib,
            log: output::log_summary(LOG_TAIL),
        }
    }
}

/// The status the slot phase fixes by itself, or `None` when the link decides (`link_status`).
/// NoNetwork → Offline, Starting → Starting, Stopping → Stopping, Up without a cache → Starting,
/// Up with a cache → `None`: the caller computes `link_status` from the live snapshot.
pub(in crate::node::node_ctl) fn phase_status(
    phase: &Phase,
    has_cache: bool,
) -> Option<NodeStatus> {
    match phase {
        Phase::NoNetwork => Some(NodeStatus::Offline),
        Phase::Starting => Some(NodeStatus::Starting),
        Phase::Stopping => Some(NodeStatus::Stopping),
        Phase::Up => (!has_cache).then_some(NodeStatus::Starting),
    }
}

/// Node status from a snapshot: online if there are network members or the relay is connected,
/// otherwise offline. `Starting`/`Stopping` are decided by `phase_status`, not here.
pub(crate) fn link_status(s: &NodeSnapshot) -> NodeStatus {
    if s.members.count > 0 || s.transport.relay.connected {
        NodeStatus::Online
    } else {
        NodeStatus::Offline
    }
}

/// Sets `now`, `node.rss_kib`, `node.rss_peak_kib`, `log`; leaves other fields alone.
pub(in crate::node::node_ctl) fn overlay_process(snap: &mut NodeSnapshot, facts: ProcessFacts) {
    snap.now = facts.uptime_secs;
    snap.node.rss_kib = facts.rss_kib;
    snap.node.rss_peak_kib = facts.rss_peak_kib;
    snap.log = facts.log;
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
