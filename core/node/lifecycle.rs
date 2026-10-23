//! The "start → run → stop or restart" loops: `run` is a thin wrapper for starting without a network
//! switch, `run_node_with_ctl`/`run_node_awaiting_key` do it with one. The stop signal belongs to the
//! process host (`host::signal`); `run` starts its own, since it has no host.

use std::sync::Arc;

use anyhow::Result;

use crate::config::config_file::Settings;
use crate::node::node_ctl::{Cmd, NodeCtl, Relaunch};
use crate::node::output::say;
use crate::proxy::ProxyConsole;
use crate::site_checks::checks;

use super::config::NodeConfig;
use super::identity::IdentityState;
use super::running::RunningNode;
use super::start::start_node;

mod command;
use command::{after_command, apply_switch, CycleEnd};

#[cfg(test)]
use crate::transport::net::PeerAddr;
use std::ops::ControlFlow;

/// Runs the "start → run → stop or restart into a new network" loop in a single process.
/// The stop signal comes from outside (the process host), so Ctrl-C during a restart is not lost.
/// The stdin console comes up only on the first cycle —
/// after a network switch it is unavailable until the process restarts.
pub async fn run_node_with_ctl(
    mut cfg: NodeConfig,
    mut settings: crate::config::config_file::Settings,
    ctl: Arc<crate::node::node_ctl::NodeCtl>,
    relaunch: crate::node::node_ctl::Relaunch,
) -> Result<()> {
    ctl.set_test_net(relaunch.test_net.clone());
    ctl.set_owner_secret(relaunch.secret.clone());
    match run_cycles(&mut cfg, &mut settings, &ctl, &relaunch).await? {
        CycleEnd::Stopped => Ok(()),
        CycleEnd::LeftNet { secret } => {
            let relaunch = forget_network(&mut settings, &ctl, &relaunch, secret);
            wait_then_run(settings, cfg.identity, &ctl, &relaunch).await
        }
    }
}

/// The node left its network: the old network's lists are dropped from the settings, the new identity goes
/// to the control object and into a copy of the start settings; the node returns to waiting for a key.
fn forget_network(
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
    Relaunch {
        secret,
        ..relaunch.clone()
    }
}

/// A node without a key: waits for the first command from the page (`wait_then_run`), then runs the
/// same `run_cycles` loop as a start with a key
/// (`run_cycles` is called directly, not `run_node_with_ctl`).
pub async fn run_node_awaiting_key(
    settings: crate::config::config_file::Settings,
    identity: IdentityState,
    ctl: Arc<crate::node::node_ctl::NodeCtl>,
    relaunch: crate::node::node_ctl::Relaunch,
) -> Result<()> {
    ctl.set_test_net(relaunch.test_net.clone());
    ctl.set_owner_secret(relaunch.secret.clone());
    wait_then_run(settings, identity, &ctl, &relaunch).await
}

/// Wait loop before the first start: `Stop` — exit (from the page or Ctrl-C/SIGTERM, already
/// turned into `Stop` by the signal task of the host); `Switch { key, peers }` — `relaunch.reload`, on success
/// — move to `run_cycles` with the invite's addresses for this start; on failure — back to
/// `NoNetwork`, keep waiting (a failure of `start_node` itself on the first cycle after that
/// is `run_cycles`' business, passed up the same way as for a start with a key).
async fn wait_then_run(
    mut settings: Settings,
    identity: IdentityState,
    ctl: &Arc<NodeCtl>,
    relaunch: &Relaunch,
) -> Result<()> {
    let mut relaunch = relaunch.clone();
    loop {
        match ctl.next_command().await {
            Cmd::Stop => return Ok(()),
            Cmd::LeaveNet { .. } => {}
            Cmd::Switch { key, peers } => {
                ctl.mark_starting();
                match apply_switch(&key, peers, identity, &mut settings, ctl, &relaunch) {
                    Ok(mut cfg) => match run_cycles(&mut cfg, &mut settings, ctl, &relaunch).await?
                    {
                        CycleEnd::Stopped => return Ok(()),
                        CycleEnd::LeftNet { secret } => {
                            relaunch = forget_network(&mut settings, ctl, &relaunch, secret);
                        }
                    },
                    Err(e) => {
                        say(
                            "node",
                            format!("{e:#}; the node is still waiting for a network"),
                        );
                        ctl.mark_no_network();
                    }
                }
            }
        }
    }
}

/// Brings up the stdin console on the first cycle: after a network switch it is
/// unavailable until the process restarts.
fn spawn_console_on_first_cycle(node: &super::running::RunningNode, ctl: &NodeCtl) {
    let console = ProxyConsole {
        front: ctl.proxy_front(),
        socks: node.socks_addr,
        exit_on: node.exit_on.clone(),
        route: node.route.clone(),
        user_agent: checks::DEFAULT_USER_AGENT.to_string(),
    };
    crate::cli::console::spawn(
        node.domains.clone(),
        node.checks.clone(),
        console,
        node.invite_source(),
    );
}

/// Cycles "start → run → stop → command handling". Split into three steps: `boot_cycle` (start),
/// `serve_until_command` (work until a command, and stop), `after_command` (Stop or network switch).
async fn run_cycles(
    cfg: &mut NodeConfig,
    settings: &mut Settings,
    ctl: &Arc<NodeCtl>,
    relaunch: &Relaunch,
) -> Result<CycleEnd> {
    let mut first_cycle = true;
    loop {
        cfg.proxy_face = ctl.proxy_face();
        apply_proxy(settings, ctl, first_cycle).await?;
        let node = boot_cycle(cfg, ctl, first_cycle).await?;
        let cmd = serve_until_command(node, ctl).await;
        if let ControlFlow::Break(end) = after_command(cmd, cfg, settings, ctl, relaunch) {
            return Ok(end);
        }
        first_cycle = false;
    }
}

/// Start of a cycle: `ctl.renew_boot_stamp()`, `start_node(cfg.clone())`; on error — `detach(Stopping)`, the line
/// "restart failed: {e:#}" only if `!first_cycle`, the error goes up; on success — the console on the first
/// cycle, `ctl.attach(&node)`, `ctl.node_up(&node)`.
async fn boot_cycle(cfg: &NodeConfig, ctl: &NodeCtl, first_cycle: bool) -> Result<RunningNode> {
    ctl.renew_boot_stamp();
    let node = match start_node(cfg.clone()).await {
        Ok(node) => node,
        Err(e) => return Err(cycle_failed(ctl, first_cycle, e)),
    };
    if first_cycle {
        spawn_console_on_first_cycle(&node, ctl);
    }
    ctl.attach(&node);
    ctl.node_up(&node);
    Ok(node)
}

/// A cycle could not start: the proxy gives up the node, `detach(Stopping)`, the line "restart failed: {e:#}"
/// only if `!first_cycle`; the error goes up.
fn cycle_failed(ctl: &NodeCtl, first_cycle: bool, e: anyhow::Error) -> anyhow::Error {
    ctl.node_down();
    ctl.detach(crate::web_gui::NodeStatus::Stopping);
    if !first_cycle {
        say("node", format!("restart failed: {e:#}"));
    }
    e
}

/// Brings the process listener to the settings before a cycle; a failure ends the cycle like a failed start.
async fn apply_proxy(settings: &Settings, ctl: &NodeCtl, first_cycle: bool) -> Result<()> {
    match ctl.apply_process_proxy(&settings.process).await {
        Ok(()) => Ok(()),
        Err(e) => Err(cycle_failed(ctl, first_cycle, e)),
    }
}

/// Work until a command: `next_command`, `detach` (Stop → Stopping, Switch → Starting), `node.shutdown().await`,
/// then the memory guard lets go of the node (its summary sees the connections while the stack closes).
async fn serve_until_command(node: RunningNode, ctl: &NodeCtl) -> Cmd {
    let cmd = ctl.next_command().await;
    ctl.detach(match &cmd {
        Cmd::Stop => crate::web_gui::NodeStatus::Stopping,
        Cmd::LeaveNet { .. } | Cmd::Switch { .. } => crate::web_gui::NodeStatus::Starting,
    });
    ctl.node_down();
    node.shutdown().await;
    ctl.release_guard_sources();
    cmd
}

/// Starts the node and runs until the stop signal (Ctrl-C or SIGTERM), then stops it.
/// A thin wrapper around `run_node_with_ctl` without a network switch — this is how the node started before network switching existed.
pub async fn run(cfg: NodeConfig) -> Result<()> {
    let settings = crate::config::config_file::Settings::default();
    let ctl = crate::node::node_ctl::NodeCtl::new(&settings);
    let secret = match &cfg.identity_secret {
        Some(s) => s.clone(),
        None => crate::config::config_file::new_secret()?,
    };
    let relaunch = crate::node::node_ctl::Relaunch {
        file: None,
        cli: crate::config::config_file::CliOverrides::default(),
        secret,
        initial_domains: cfg.initial_domains.clone(),
        blocked_domains: cfg.blocked_domains.clone(),
        test_net: cfg.test_net.clone(),
    };
    let signal_task = super::host::signal::spawn_stop_signal({
        let ctl = ctl.clone();
        move || ctl.stop_on_os_signal()
    });
    let result = run_node_with_ctl(cfg, settings, ctl, relaunch).await;
    signal_task.abort();
    result
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
