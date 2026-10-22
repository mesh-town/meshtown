//! Live settings paths: which of `Settings` apply to an already running node without a
//! restart, and how. Used by both `state::attach` (deferred paths) and `control::apply_config`
//! (freshly computed ones).

use std::time::Duration;

use crate::config_file::fields;
use crate::config_file::Settings;
use crate::node::RunningNode;
use crate::output;

/// Journal ring capacity from the settings (at least one line); shared by the
/// `log.ring_lines` handle, `Switch` and `control_node`.
pub(crate) fn apply_log_capacity(s: &Settings) {
    output::set_log_capacity((s.process.log_ring_lines as usize).max(1));
}

/// A live-path handle: carries a value from `effective_settings` into the running node.
type Handle = fn(&RunningNode, &Settings);

/// Live-setting handles by path; the set of paths equals `fields::live_paths()` (test `handles_equal_live`).
const HANDLES: [(&str, Handle); 9] = [
    ("exit.enabled", |n, e| n.set_exit_enabled(e.net.exit_enabled)),
    ("checks.enabled", |n, e| n.set_checks_enabled(e.net.checks_enabled)),
    ("checks.interval_sec", |n, e| {
        n.set_check_interval(Duration::from_secs(e.net.check_interval_sec.max(1)))
    }),
    ("probe.interval_sec", |n, e| {
        n.set_probe_interval(Duration::from_secs(e.net.probe_interval_sec.max(1)))
    }),
    ("node.name", |n, e| n.set_node_name(e.net.node_name.as_deref())),
    ("route.exits", |n, e| n.set_route_exits(e.net.route_exits.clone())),
    ("ports.published", |n, e| n.set_ports_published(e.net.ports_published.clone())),
    ("ports.opened", |n, e| n.set_ports_opened(e.net.ports_opened.clone())),
    ("log.ring_lines", |_, e| apply_log_capacity(e)),
];

/// Copies into `effective_settings` only the live ones from `paths`; the rest leave `effective_settings` untouched and go to the second
/// list — "take effect after a restart". Does not take the lock.
pub(super) fn copy_live_paths(
    effective_settings: &mut Settings,
    from: &Settings,
    paths: &[&'static str],
) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut live = Vec::new();
    let mut deferred = Vec::new();
    for &p in paths {
        if let Some(f) = fields::field_by_path(p).filter(|_| fields::live_paths().contains(&p)) {
            f.value.copy_value_between(from, effective_settings);
            live.push(p);
        } else {
            deferred.push(p);
        }
    }
    (live, deferred)
}

/// Applies live paths to an already running node through the live-setting handles (shared by `attach` — with `pending` —
/// and `apply_config` — with the freshly computed `live`). `log.ring_lines` is a global handle,
/// not tied to the node; `node.name` goes to `set_node_name` (handshakes and the network document).
pub(super) fn apply_pending_handles(node: &RunningNode, paths: &[&'static str], effective_settings: &Settings) {
    for &p in paths {
        if let Some((_, handle)) = HANDLES.iter().find(|(path, _)| *path == p) {
            handle(node, effective_settings);
        }
    }
}

#[cfg(test)]
#[path = "live_paths_tests.rs"]
mod tests;
