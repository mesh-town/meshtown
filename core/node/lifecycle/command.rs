//! Handling of the command that ended a node cycle: stop, or switch to another network.

use std::ops::ControlFlow;

use anyhow::Result;

use crate::config::config_file::Settings;
use crate::node::node_ctl::{Cmd, NodeCtl, Relaunch};
use crate::node::output::say;
use crate::transport::net::PeerAddr;

use super::super::config::NodeConfig;
use super::super::identity::IdentityState;

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

/// After the stop: Stop → `Break`; Switch → `apply_switch` with the old `cfg.identity`, on success `*cfg = new`,
/// on error the line "coming back up with the old key"; then `Break` if a stop was requested meanwhile, else `Continue`.
pub(super) fn after_command(
    cmd: Cmd,
    cfg: &mut NodeConfig,
    settings: &mut Settings,
    ctl: &NodeCtl,
    relaunch: &Relaunch,
) -> ControlFlow<()> {
    let Cmd::Switch { key, peers } = cmd else {
        return ControlFlow::Break(());
    };
    match apply_switch(&key, peers, cfg.identity, settings, ctl, relaunch) {
        Ok(new_cfg) => *cfg = new_cfg,
        Err(e) => say("node", format!("{e:#}; coming back up with the old key")),
    }
    if ctl.stop_requested() {
        return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
