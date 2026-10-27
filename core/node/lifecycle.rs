//! The "start → run → stop or restart" loops: `run` is a thin wrapper for starting without a network
//! switch, `run_node_with_ctl`/`run_node_awaiting_key` do it with one, and `run_network` picks between
//! them by whether the first key is already known. The stop signal belongs to the process host
//! (`host::signal`); `run` starts its own, since it has no host.

use std::sync::Arc;

use anyhow::Result;

use crate::config::config_file::{RunConfig, Settings};
use crate::node::node_ctl::{Cmd, NodeCtl, Relaunch};
use crate::node::output::say;
use crate::proxy::ProxyConsole;
use crate::site_checks::checks;

use super::config::NodeConfig;
use super::identity::IdentityState;
use super::running::RunningNode;
use super::start::start_node;
use start_race::start_unless_stopped;

mod command;
mod start_race;
use command::{after_command, apply_switch, fall_back_to_waiting, forget_network, CycleEnd};

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
        CycleEnd::StartFailed(e) => {
            fall_back_to_waiting(&ctl, &e);
            wait_then_run(settings, cfg.identity, &ctl, &relaunch).await
        }
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

/// Runs one network's node until it stops: with `Some(cfg)` it starts at once, with `None` it waits for the
/// first key from the page.
pub async fn run_network(
    start: Option<NodeConfig>,
    run: RunConfig,
    ctl: Arc<NodeCtl>,
    relaunch: Relaunch,
) -> Result<()> {
    match start {
        Some(cfg) => run_node_with_ctl(cfg, run.effective_settings, ctl, relaunch).await,
        None => {
            let identity = IdentityState::from_report(&run.file_report);
            run_node_awaiting_key(run.effective_settings, identity, ctl, relaunch).await
        }
    }
}

/// Wait loop before the first start: a `Stop` that lands while a switch is applied also exits before the boot. `Stop` — exit (from the page or Ctrl-C/SIGTERM, already
/// turned into `Stop` by the signal task of the host); `LeaveNet` — forget the network and keep waiting;
/// `Switch { key, peers }` — `relaunch.reload`, on success — move to `run_cycles` with the invite's addresses
/// for this start; on failure — back to `NoNetwork`, keep waiting. A start that fails (an error or
/// `StartFailed`) also falls back to `NoNetwork` with the reason and keeps waiting.
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
            Cmd::LeaveNet { secret } => {
                relaunch = forget_network(&mut settings, ctl, &relaunch, secret);
            }
            Cmd::Switch { key, peers } => {
                ctl.mark_starting();
                match apply_switch(&key, peers, identity, &mut settings, ctl, &relaunch) {
                    Ok(_) if ctl.stop_requested() => return Ok(()),
                    Ok(mut cfg) => {
                        match run_cycles(&mut cfg, &mut settings, ctl, &relaunch).await {
                            Ok(CycleEnd::Stopped) => return Ok(()),
                            Ok(CycleEnd::LeftNet { secret }) => {
                                relaunch = forget_network(&mut settings, ctl, &relaunch, secret);
                            }
                            Ok(CycleEnd::StartFailed(e)) | Err(e) => fall_back_to_waiting(ctl, &e),
                        }
                    }
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
        exit_on: node.exit_on.clone(),
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
/// A stop that arrived before a cycle begins ends the loop without booting the node.
async fn run_cycles(
    cfg: &mut NodeConfig,
    settings: &mut Settings,
    ctl: &Arc<NodeCtl>,
    relaunch: &Relaunch,
) -> Result<CycleEnd> {
    let mut first_cycle = true;
    loop {
        if ctl.stop_requested() {
            return Ok(CycleEnd::Stopped);
        }
        cfg.proxy_face = ctl.proxy_face();
        let node = match start_cycle(cfg, settings, ctl, first_cycle).await {
            Ok(Some(node)) => node,
            Ok(None) => return Ok(CycleEnd::Stopped),
            Err(e) if first_cycle => return Err(e),
            Err(e) => return Ok(CycleEnd::StartFailed(e)),
        };
        let cmd = serve_until_command(node, ctl).await;
        if let ControlFlow::Break(end) = after_command(cmd, cfg, settings, ctl, relaunch) {
            return Ok(end);
        }
        first_cycle = false;
    }
}

/// The proxy to the settings, then the boot: an error of either ends the cycle as a failed start.
async fn start_cycle(
    cfg: &NodeConfig,
    settings: &Settings,
    ctl: &NodeCtl,
    first_cycle: bool,
) -> Result<Option<RunningNode>> {
    apply_proxy(settings, ctl).await?;
    boot_cycle(cfg, ctl, first_cycle).await
}

/// Start of a cycle: `start_node(cfg.clone())` through `boot_cycle_with`.
async fn boot_cycle(
    cfg: &NodeConfig,
    ctl: &NodeCtl,
    first_cycle: bool,
) -> Result<Option<RunningNode>> {
    boot_cycle_with(ctl, first_cycle, start_node(cfg.clone())).await
}

/// Start of a cycle: `ctl.renew_boot_stamp()`, the start itself (a stop request interrupts it: `None`, the
/// start is closed in the background); on error — `detach(Stopping)`, the error goes up; on success — the console on the first cycle, `ctl.attach(&node)`,
/// `ctl.node_up(&node)`.
async fn boot_cycle_with(
    ctl: &NodeCtl,
    first_cycle: bool,
    start: impl std::future::Future<Output = Result<RunningNode>> + Send + 'static,
) -> Result<Option<RunningNode>> {
    ctl.renew_boot_stamp();
    let started = start_unless_stopped(ctl, start, |node| Box::pin(node.discard())).await;
    let node = match started {
        Ok(Some(node)) => node,
        Ok(None) => {
            say("node", "start interrupted by a stop");
            return Ok(None);
        }
        Err(e) => return Err(cycle_failed(ctl, e)),
    };
    if first_cycle {
        spawn_console_on_first_cycle(&node, ctl);
    }
    ctl.attach(&node);
    ctl.node_up(&node);
    Ok(Some(node))
}

/// A cycle could not start: the proxy gives up the node, `detach(Stopping)`; the error goes up.
fn cycle_failed(ctl: &NodeCtl, e: anyhow::Error) -> anyhow::Error {
    ctl.node_down();
    ctl.detach(crate::web_gui::NodeStatus::Stopping);
    e
}

/// Brings the process listener to the settings before a cycle; a failure ends the cycle like a failed start.
async fn apply_proxy(settings: &Settings, ctl: &NodeCtl) -> Result<()> {
    match ctl.apply_process_proxy(&settings.process).await {
        Ok(()) => Ok(()),
        Err(e) => Err(cycle_failed(ctl, e)),
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
#[path = "lifecycle_start_stop_tests.rs"]
mod start_stop_tests;
#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
