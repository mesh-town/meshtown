//! Live settings paths: which of `Settings` apply to an already running node without a
//! restart, and how. Used by both `state::attach` (deferred paths) and `control::apply_config`
//! (freshly computed ones).

use std::time::Duration;

use crate::config::config_file::fields;
use crate::config::config_file::Settings;
use crate::exit::ExitLimits;
use crate::node::output;
use crate::node::RunningNode;

/// Journal ring capacity from the settings (at least one line); shared by the
/// `log.ring_lines` handle, `Switch` and `control_node`.
pub(crate) fn apply_log_capacity(s: &Settings) {
    output::set_log_capacity((s.process.log_ring_lines as usize).max(1));
}

/// Sets the exit stream limit from the settings (`0` means the machine-memory default); open streams stay.
pub(super) fn apply_exit_max_streams(limits: &ExitLimits, s: &Settings) {
    limits.set_max_streams(s.process.exit_max_streams);
}

/// Hands `socks.strict` to `set` (the PAC flag of the proxy front); a parameter keeps tests off the global flag.
pub(super) fn apply_pac_strict(s: &Settings, set: impl FnOnce(bool)) {
    set(s.process.socks_strict);
}

/// A live-path handle: carries a value from `effective_settings` into the running node.
type Handle = fn(&RunningNode, &Settings);

/// Live-setting handles by path; the set of paths equals `fields::live_paths()` (test `handles_equal_live`).
const HANDLES: [(&str, Handle); 13] = [
    ("exit.enabled", |n, e| {
        n.set_exit_enabled(e.net.exit_enabled)
    }),
    ("exit.interface", |n, e| {
        n.set_exit_interface(e.net.exit_interface.as_deref())
    }),
    ("checks.enabled", |n, e| {
        n.set_checks_enabled(e.net.checks_enabled)
    }),
    ("checks.interval_sec", |n, e| {
        n.set_check_interval(Duration::from_secs(e.net.check_interval_sec.max(1)))
    }),
    ("probe.interval_sec", |n, e| {
        n.set_probe_interval(Duration::from_secs(e.net.probe_interval_sec.max(1)))
    }),
    ("node.name", |n, e| {
        n.set_node_name(e.net.node_name.as_deref())
    }),
    ("route.exits", |n, e| {
        n.set_route_exits(e.net.route_exits.clone())
    }),
    ("ports.published", |n, e| {
        n.set_ports_published(e.net.ports_published.clone())
    }),
    ("ports.opened", |n, e| {
        n.set_ports_opened(e.net.ports_opened.clone())
    }),
    ("log.ring_lines", |_, e| apply_log_capacity(e)),
    // A process-wide handle, not the node's: the stream limit is shared by every network.
    ("exit.max_streams", |_, e| {
        apply_exit_max_streams(&ExitLimits::process(), e)
    }),
    // A process-wide flag, not the node's: the PAC of the one proxy front reads it on every request.
    ("socks.strict", |_, e| {
        apply_pac_strict(e, crate::proxy::set_pac_strict)
    }),
    // The proxy front reads the setting itself; nothing to push into the node.
    ("proxy.net", |_, _| {}),
];

/// Copies into `effective_settings` only the live ones from `paths`; the rest leave `effective_settings` untouched and go to the second
/// list — "take effect after a restart". Does not take the lock.
pub(super) fn copy_live_paths(
    effective_settings: &mut Settings,
    from: &Settings,
    paths: &[&'static str],
) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut live = Vec::new();
    for &p in paths {
        if let Some(f) = fields::field_by_path(p).filter(|_| is_live(p)) {
            f.value.copy_value_between(from, effective_settings);
            live.push(p);
        }
    }
    (live, deferred_paths(paths))
}

/// Whether the field behind the path exists and takes effect without a restart.
fn is_live(path: &'static str) -> bool {
    fields::field_by_path(path).is_some() && fields::live_paths().contains(&path)
}

/// The paths that take effect only after a restart, in the order given.
fn deferred_paths(paths: &[&'static str]) -> Vec<&'static str> {
    paths.iter().copied().filter(|&p| !is_live(p)).collect()
}

/// Applies live paths to an already running node through the live-setting handles (shared by `attach` — with `pending` —
/// and `apply_config` — with the freshly computed `live`). `log.ring_lines` is a global handle,
/// not tied to the node; `node.name` goes to `set_node_name` (handshakes and the network document).
pub(super) fn apply_pending_handles(
    node: &RunningNode,
    paths: &[&'static str],
    effective_settings: &Settings,
) {
    for &p in paths {
        if let Some((_, handle)) = HANDLES.iter().find(|(path, _)| *path == p) {
            handle(node, effective_settings);
        }
    }
}

#[cfg(test)]
#[path = "live_paths_tests.rs"]
mod tests;
