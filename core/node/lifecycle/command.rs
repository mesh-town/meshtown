//! Handling of the command that ended a node cycle: stop, or switch to another network.

use std::ops::ControlFlow;

use anyhow::Result;

use crate::config::config_file::Settings;
use crate::node::node_ctl::{Cmd, NodeCtl, Relaunch};
use crate::node::output::say;
use crate::transport::net::{NodeSecret, PeerAddr};

use super::super::config::NodeConfig;
use super::super::identity::IdentityState;

/// Why the cycle loop ended: a stop, or the node left its network and takes a new identity.
pub(super) enum CycleEnd {
    Stopped,
    LeftNet {
        secret: NodeSecret,
    },
    /// A cycle after the first could not start; the caller falls back to waiting for a network.
    StartFailed(anyhow::Error),
}

/// A start failed and the process stays up: one log line, then the node waits for a network again with the
/// reason kept for the page.
pub(super) fn fall_back_to_waiting(ctl: &NodeCtl, e: &anyhow::Error) {
    say(
        "node",
        format!("start failed: {e:#}; the node is still waiting for a network"),
    );
    ctl.fall_back_to_no_network(format!("{e:#}"));
}

/// Handles `Switch`: `relaunch.reload`, on success — the new settings into `settings`, a new cfg with the passed
/// `identity` and `initial_peers = peers`, `ctl.replace_effective_settings`, the ring capacity. On error changes nothing.
pub(super) fn apply_switch(
    key: &str,
    peers: Vec<PeerAddr>,
    identity: IdentityState,
    settings: &mut Settings,
    ctl: &NodeCtl,
    relaunch: &Relaunch,
) -> Result<NodeConfig> {
    let (s, mut cfg) = relaunch.reload(key, settings)?;
    *settings = s;
    cfg.identity = identity;
    cfg.initial_peers = peers;
    ctl.replace_effective_settings(settings);
    crate::node::node_ctl::apply_log_capacity(settings);
    Ok(cfg)
}

/// After the stop: Stop → `Break(Stopped)`; LeaveNet → `Break(LeftNet)`; Switch → `apply_switch` with the old `cfg.identity`, on success `*cfg = new`,
/// on error the line "coming back up with the old key"; then `Break` if a stop was requested meanwhile, else `Continue`.
pub(super) fn after_command(
    cmd: Cmd,
    cfg: &mut NodeConfig,
    settings: &mut Settings,
    ctl: &NodeCtl,
    relaunch: &Relaunch,
) -> ControlFlow<CycleEnd> {
    let (key, peers) = match cmd {
        Cmd::Stop => return ControlFlow::Break(CycleEnd::Stopped),
        Cmd::LeaveNet { secret } => return ControlFlow::Break(CycleEnd::LeftNet { secret }),
        Cmd::Switch { key, peers } => (key, peers),
    };
    match apply_switch(&key, peers, cfg.identity, settings, ctl, relaunch) {
        Ok(new_cfg) => *cfg = new_cfg,
        Err(e) => say("node", format!("{e:#}; coming back up with the old key")),
    }
    if ctl.stop_requested() {
        return ControlFlow::Break(CycleEnd::Stopped);
    }
    ControlFlow::Continue(())
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;

/// The node left its network: the old lists and the remembered node id are dropped, the new identity goes
/// to the control object and the start settings copy; the node returns to waiting for a key.
pub(super) fn forget_network(
    settings: &mut Settings,
    ctl: &NodeCtl,
    relaunch: &Relaunch,
    secret: crate::transport::net::NodeSecret,
) -> Relaunch {
    settings.net.network_key = None;
    settings.net.route_exits.clear();
    settings.net.ports_published.clear();
    ctl.set_owner_secret(secret.clone());
    ctl.replace_effective_settings(settings);
    ctl.mark_no_network();
    ctl.forget_node_id();
    Relaunch {
        secret,
        ..relaunch.clone()
    }
}
