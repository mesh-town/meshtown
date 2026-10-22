//! The stop signal and the "start → run → stop or restart" loops: `run` is
//! a thin wrapper for starting without a network switch, `run_node_with_ctl`/`run_node_awaiting_key` do it with one.

use std::ops::ControlFlow;
use std::sync::Arc;

use anyhow::Result;

use crate::checks;
use crate::config_file::Settings;
use crate::net::PeerAddr;
use crate::node_ctl::{Cmd, NodeCtl, Relaunch};
use crate::output::say;
use crate::proxy::ProxyConsole;

use super::config::NodeConfig;
use super::identity::IdentityState;
use super::running::RunningNode;
use super::start::start_node;

#[cfg(unix)]
async fn wait_for_stop_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(_) => {
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = term.recv() => {},
    }
}

#[cfg(not(unix))]
async fn wait_for_stop_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Stop signal task: waits for Ctrl-C/SIGTERM, calls `ctl.stop_on_os_signal()`. One for the whole call of
/// `run_node_with_ctl`/`run_node_awaiting_key` — otherwise either two signal tasks (a double line and two `Stop`s),
/// or a gap between waiting and the node's work in which Ctrl-C would be lost.
fn spawn_stop_signal(ctl: Arc<crate::node_ctl::NodeCtl>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        wait_for_stop_signal().await;
        say("node", "received stop signal");
        ctl.stop_on_os_signal();
    })
}

/// Runs the "start → run → stop or restart into a new network" loop in a single process.
/// There is one signal task for the whole call (otherwise Ctrl-C during a restart would be lost) and it
/// is aborted on any exit. The stdin console comes up only on the first cycle —
/// after a network switch it is unavailable until the process restarts.
pub async fn run_node_with_ctl(
    mut cfg: NodeConfig,
    mut settings: crate::config_file::Settings,
    ctl: Arc<crate::node_ctl::NodeCtl>,
    relaunch: crate::node_ctl::Relaunch,
) -> Result<()> {
    ctl.set_test_net(relaunch.test_net.clone());
    ctl.set_owner_secret(relaunch.secret.clone());
    let signal_task = spawn_stop_signal(ctl.clone());
    let result = run_cycles(&mut cfg, &mut settings, &ctl, &relaunch).await;
    signal_task.abort();
    result
}

/// A node without a key: waits for the first command from the page (`wait_then_run`), then runs the
/// same `run_cycles` loop as a start with a key, under one signal task
/// (`run_cycles` is called directly, not `run_node_with_ctl`).
pub async fn run_node_awaiting_key(
    settings: crate::config_file::Settings,
    identity: IdentityState,
    ctl: Arc<crate::node_ctl::NodeCtl>,
    relaunch: crate::node_ctl::Relaunch,
) -> Result<()> {
    ctl.set_test_net(relaunch.test_net.clone());
    ctl.set_owner_secret(relaunch.secret.clone());
    let signal_task = spawn_stop_signal(ctl.clone());
    let result = wait_then_run(settings, identity, &ctl, &relaunch).await;
    signal_task.abort();
    result
}

/// Wait loop before the first start: `Stop` — exit (from the page or Ctrl-C/SIGTERM, already
/// turned into `Stop` by the signal task); `Switch { key, peers }` — `relaunch.reload`, on success
/// — move to `run_cycles` with the invite's addresses for this start; on failure — back to
/// `NoNetwork`, keep waiting (a failure of `start_node` itself on the first cycle after that
/// is `run_cycles`' business, passed up the same way as for a start with a key).
async fn wait_then_run(
    mut settings: Settings,
    identity: IdentityState,
    ctl: &Arc<NodeCtl>,
    relaunch: &Relaunch,
) -> Result<()> {
    loop {
        match ctl.next_command().await {
            Cmd::Stop => return Ok(()),
            Cmd::Switch { key, peers } => {
                ctl.mark_starting();
                match apply_switch(&key, peers, identity, &mut settings, ctl, relaunch) {
                    Ok(mut cfg) => return run_cycles(&mut cfg, &mut settings, ctl, relaunch).await,
                    Err(e) => {
                        say("node", format!("{e:#}; the node is still waiting for a network"));
                        ctl.mark_no_network();
                    }
                }
            }
        }
    }
}

/// Handles `Switch`: `relaunch.reload`, on success — the new settings into `settings`, a new cfg with the passed
/// `identity` and `initial_peers = peers`, `ctl.replace_effective_settings`, the ring capacity. On error changes nothing.
fn apply_switch(
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
    crate::node_ctl::apply_log_capacity(settings);
    Ok(cfg)
}

/// Brings up the stdin console on the first cycle: after a network switch it is
/// unavailable until the process restarts.
fn spawn_console_on_first_cycle(node: &super::running::RunningNode) {
    let console = ProxyConsole {
        socks: node.socks_addr,
        exit_on: node.exit_on.clone(),
        route: node.route.clone(),
        user_agent: checks::DEFAULT_USER_AGENT.to_string(),
    };
    crate::console::spawn(
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
) -> Result<()> {
    let mut first_cycle = true;
    loop {
        let node = boot_cycle(cfg, ctl, first_cycle).await?;
        let cmd = serve_until_command(node, ctl).await;
        if after_command(cmd, cfg, settings, ctl, relaunch).is_break() {
            return Ok(());
        }
        first_cycle = false;
    }
}

/// Start of a cycle: `ctl.renew_boot_stamp()`, `start_node(cfg.clone())`; on error — `detach(Stopping)`, the line
/// "restart failed: {e:#}" only if `!first_cycle`, the error goes up; on success — the console on the first
/// cycle, `ctl.attach(&node)`.
async fn boot_cycle(cfg: &NodeConfig, ctl: &NodeCtl, first_cycle: bool) -> Result<RunningNode> {
    ctl.renew_boot_stamp();
    let node = match start_node(cfg.clone()).await {
        Ok(node) => node,
        Err(e) => {
            ctl.detach(crate::web::NodeStatus::Stopping);
            if !first_cycle {
                say("node", format!("restart failed: {e:#}"));
            }
            return Err(e);
        }
    };
    if first_cycle {
        spawn_console_on_first_cycle(&node);
    }
    ctl.attach(&node);
    Ok(node)
}

/// Work until a command: `next_command`, `detach` (Stop → Stopping, Switch → Starting), `node.shutdown().await`.
async fn serve_until_command(node: RunningNode, ctl: &NodeCtl) -> Cmd {
    let cmd = ctl.next_command().await;
    ctl.detach(match &cmd {
        Cmd::Stop => crate::web::NodeStatus::Stopping,
        Cmd::Switch { .. } => crate::web::NodeStatus::Starting,
    });
    node.shutdown().await;
    cmd
}

/// After the stop: Stop → `Break`; Switch → `apply_switch` with the old `cfg.identity`, on success `*cfg = new`,
/// on error the line "coming back up with the old key"; then `Continue`.
fn after_command(
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
    ControlFlow::Continue(())
}

/// Starts the node and runs until the stop signal (Ctrl-C or SIGTERM), then stops it.
/// A thin wrapper around `run_node_with_ctl` without a network switch — this is how the node started before network switching existed.
pub async fn run(cfg: NodeConfig) -> Result<()> {
    let settings = crate::config_file::Settings::default();
    let ctl = crate::node_ctl::NodeCtl::new(&settings);
    let secret = match &cfg.identity_secret {
        Some(s) => s.clone(),
        None => crate::config_file::new_secret()?,
    };
    let relaunch = crate::node_ctl::Relaunch {
        path: None,
        cli: crate::config_file::CliOverrides::default(),
        secret,
        initial_domains: cfg.initial_domains.clone(),
        blocked_domains: cfg.blocked_domains.clone(),
        test_net: cfg.test_net.clone(),
    };
    run_node_with_ctl(cfg, settings, ctl, relaunch).await
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
