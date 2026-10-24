//! Node state under the `attach`/`detach` cycle: the phase (`Phase`), queue commands (`Cmd`), `NodeCtl`
//! and everything under the `NodeCtlState`/`NodeCtlShared` lock. Page actions (`web::NodeControl`) are in `control.rs`;
//! the background snapshot collection is in `task.rs`; live settings paths are in `live_paths.rs`.

use std::sync::Arc;

use tokio::runtime::Handle;
use tokio::sync::{mpsc, watch, Notify};
use tokio::time::Instant;

use crate::config::config_file::Settings;
use crate::members::access::tally::Closed;
use crate::node::RunningNode;
use crate::transport::net::PeerAddr;
use crate::web_gui::{ActionError, NodeStatus};

use super::closed_feed::abort_task;
use super::invite_wait::InviteWait;
use super::live_paths::apply_pending_handles;
use super::snapshot::Collected;
use super::task::snapshot_task;

/// The node cycle phase. `Up` — the node is up (the snapshot may not be ready right after `attach`).
/// `NoNetwork` — a node without a key: waits for the first page command (`run_node_awaiting_key`).
pub(super) enum Phase {
    NoNetwork,
    Starting,
    Up,
    Stopping,
}

/// A command taken from the queue for `run_node_with_ctl`/`run_node_awaiting_key`. `Stop` always wins;
/// of several `Switch` commands in a row the last one stays whole, with its addresses.
/// Comparison is by hand (`leave_net.rs`): the node secret has no equality.
#[derive(Clone, Debug)]
pub(crate) enum Cmd {
    Stop,
    LeaveNet {
        secret: crate::transport::net::NodeSecret,
    },
    Switch {
        key: String,
        peers: Vec<PeerAddr>,
    },
}

/// State under one lock. Under the lock — only assignments and clones: no `.await`,
/// no `say`, `output::*`, `telemetry::*`, live-setting handles (all of them write to the log or wait).
pub(super) struct NodeCtlState {
    pub(super) phase: Phase,
    /// Stamp of the current cycle: `boot_id` and the start of `uptime_sec`; changed only by `renew_boot_stamp`.
    pub(super) boot: super::boot_stamp::BootStamp,
    pub(super) node: Option<Arc<RunningNode>>,
    /// Effective settings: the file + flags + whatever was applied through `apply_config`/`reload`.
    pub(super) effective_settings: Settings,
    /// Live paths that arrived through `apply_config` without a node — applied in `attach`.
    pub(super) pending: Vec<&'static str>,
    /// `None` — no collection has happened since the last `attach`.
    pub(super) cache: Option<Collected>,
    /// Snapshot task generation: `+1` in `attach` and `detach`, so that an old task does not write
    /// into the new node's `cache`.
    pub(super) snapshot_gen: u64,
    pub(super) task: Option<tokio::task::JoinHandle<()>>,
    /// Forwards the access object's `closed` value into `NodeCtlShared.closed`; replaced by `attach`.
    pub(super) closed_task: Option<tokio::task::JoinHandle<()>>,
    /// The last `snapshot()` call — the background task uses it to decide whether to skip a collection.
    pub(super) last_read: Instant,
    /// Joining by invite: waiting or the last refusal (`invite_wait.rs`).
    pub(super) invite_wait: Option<InviteWait>,
    /// Wait generation: `+1` for every new join by invite.
    pub(super) invite_gen: u64,
    /// The `mem` test network (from `Relaunch.test_net`): a temporary transport for joining by invite.
    pub(super) test_net: Option<Arc<crate::transport::net::mem::MemNet>>,
    /// The node's permanent key (from `Relaunch.secret`): proves ownership when joining by invite.
    pub(super) owner: Option<crate::transport::net::NodeSecret>,
}

pub(super) struct NodeCtlShared {
    pub(super) ctl_state: std::sync::Mutex<NodeCtlState>,
    /// Wakes the background snapshot task earlier than the 1 s tick. Holds one permit — a nudge without
    /// a waiter is not lost.
    pub(super) kick: Notify,
    /// The slot's permanent "network closed" channel: it outlives the node cycles.
    pub(super) closed: watch::Sender<Option<Closed>>,
}

/// Lives in an `Arc`: the web listener, `run_node_with_ctl` and the background snapshot task each hold their own copy.
pub struct NodeCtl {
    pub(super) rt: Handle,
    pub(super) ctl_shared: Arc<NodeCtlShared>,
    pub(super) cmd_tx: mpsc::UnboundedSender<Cmd>,
    /// The receiver is used only under `next_command`, which only `run_node_with_ctl` calls;
    /// holding the lock across `.await` is legal here (it is not a `std::sync::Mutex`).
    cmd_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<Cmd>>,
    /// Where the process memory guard reads the live node from; unset when there is no process host.
    pub(super) guard_sources: std::sync::OnceLock<(
        crate::config::config_file::NetSlotId,
        crate::node::guard_sources::GuardSources,
    )>,
    /// The process proxy seat and the slot id the node publishes under; unset when there is no process host.
    pub(super) proxy: std::sync::OnceLock<(
        crate::config::config_file::NetSlotId,
        Arc<crate::proxy::ProxySeat>,
    )>,
}

impl NodeCtl {
    /// `rt = Handle::current()` — called from the body of `run`/`run_node_with_ctl`/`run_node_awaiting_key` on the
    /// tokio.
    pub fn new(settings: &Settings) -> Arc<NodeCtl> {
        Self::with_phase(settings, Phase::Starting)
    }

    /// A node without a network key: waits for the first page command (`node::run_node_awaiting_key`).
    pub fn new_without_network(settings: &Settings) -> Arc<NodeCtl> {
        Self::with_phase(settings, Phase::NoNetwork)
    }

    fn with_phase(settings: &Settings, phase: Phase) -> Arc<NodeCtl> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        Arc::new(NodeCtl {
            rt: Handle::current(),
            ctl_shared: Arc::new(NodeCtlShared {
                ctl_state: std::sync::Mutex::new(NodeCtlState {
                    phase,
                    boot: super::boot_stamp::BootStamp::new(),
                    node: None,
                    effective_settings: settings.clone(),
                    pending: Vec::new(),
                    cache: None,
                    snapshot_gen: 0,
                    task: None,
                    closed_task: None,
                    last_read: Instant::now(),
                    invite_wait: None,
                    invite_gen: 0,
                    test_net: None,
                    owner: None,
                }),
                kick: Notify::new(),
                closed: watch::channel(None).0,
            }),
            cmd_tx,
            cmd_rx: tokio::sync::Mutex::new(cmd_rx),
            guard_sources: std::sync::OnceLock::new(),
            proxy: std::sync::OnceLock::new(),
        })
    }

    /// Remembers the `mem` test network for the temporary invite-join transport.
    #[doc(hidden)]
    pub fn set_test_net(&self, net: Option<Arc<crate::transport::net::mem::MemNet>>) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.test_net = net;
    }

    /// Remembers the node's permanent key for joining by invite.
    pub fn set_owner_secret(&self, secret: crate::transport::net::NodeSecret) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.owner = Some(secret);
    }

    /// The node is up: the snapshot task generation grows, deferred live paths are applied,
    /// and the background collection starts.
    pub(crate) fn attach(&self, node: &RunningNode) {
        let node = Arc::new(node.clone());
        let (task_gen, pending, effective_settings) = {
            let mut st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            st.snapshot_gen += 1;
            abort_task(&mut st.closed_task);
            st.node = Some(node.clone());
            st.cache = None;
            if !matches!(st.phase, Phase::Stopping) {
                st.phase = Phase::Up;
            }
            let pending = std::mem::take(&mut st.pending);
            (st.snapshot_gen, pending, st.effective_settings.clone())
        };
        apply_pending_handles(&node, &pending, &effective_settings);
        self.publish_guard_sources(node.members.clone(), node.checks.clone());
        let task = crate::node::output::spawn_on(
            &self.rt,
            snapshot_task(self.ctl_shared.clone(), task_gen, node.clone()),
        );
        let closed_task = self.start_closed_feed(node.access.clone());
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.task = Some(task);
        st.closed_task = Some(closed_task);
    }

    /// The node goes away: the snapshot task is aborted, the state is cleared, the phase follows `next`
    /// (`Starting` — a new restart cycle, otherwise `Stopping`).
    pub(crate) fn detach(&self, next: NodeStatus) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.snapshot_gen += 1;
        if let Some(task) = st.task.take() {
            task.abort();
        }
        st.node = None;
        st.cache = None;
        st.pending.clear();
        abort_task(&mut st.closed_task);
        self.ctl_shared.closed.send_replace(None);
        st.phase = if matches!(next, NodeStatus::Starting) {
            Phase::Starting
        } else {
            Phase::Stopping
        };
    }

    /// Waits for the next command for `run_node_with_ctl`: `recv().await` first, then everything in the queue
    /// is folded — `Stop` wins, then `LeaveNet`, otherwise the last `Switch` stays.
    pub(crate) async fn next_command(&self) -> Cmd {
        let mut rx = self.cmd_rx.lock().await;
        let mut cmd = rx.recv().await.expect("cmd_tx lives as long as NodeCtl");
        while let Ok(next) = rx.try_recv() {
            cmd = match (cmd, next) {
                (_, Cmd::Stop) => Cmd::Stop,
                (Cmd::Stop, _) => Cmd::Stop,
                (leave @ Cmd::LeaveNet { .. }, Cmd::Switch { .. }) => leave,
                (_, other) => other,
            };
        }
        cmd
    }

    /// After a successful `reload`: the effective settings are replaced with the new ones, deferred live
    /// paths are dropped (the new node is not up yet — there is nothing to apply them to).
    pub(crate) fn replace_effective_settings(&self, settings: &Settings) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.effective_settings = settings.clone();
        st.pending.clear();
    }

    /// The same transition as `stop()`, but without the log line about a page request — called by the signal
    /// task of the process host on Ctrl-C/SIGTERM.
    pub(crate) fn stop_on_os_signal(&self) {
        self.request_stop();
    }

    pub(super) fn request_stop(&self) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.phase = Phase::Stopping;
        super::invite_wait::cancel(&mut st);
        drop(st);
        let _ = self.cmd_tx.send(Cmd::Stop);
    }

    /// Restart in the same process: only from `Up`, `Cmd::Switch` with the current key (the join path
    /// rereads the config file and applies the CLI flags). Other phases — `Busy`.
    pub(super) fn request_restart(&self) -> Result<(), ActionError> {
        let key = {
            let mut st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match st.phase {
                Phase::Up => {}
                Phase::NoNetwork => {
                    return Err(ActionError::NoNetwork(format!(
                        "the node has not joined the {0} network yet: paste an invite from a network member or create your own",
                        crate::node::output::NET_NAME
                    )))
                }
                Phase::Starting => return Err(ActionError::Busy("node is starting".to_string())),
                Phase::Stopping => {
                    return Err(ActionError::Busy("node is stopping".to_string()))
                }
            }
            let Some(key) = st.effective_settings.net.network_key.clone() else {
                return Err(ActionError::Internal("network key is unknown".to_string()));
            };
            st.phase = Phase::Starting;
            key
        };
        let _ = self.cmd_tx.send(Cmd::Switch { key, peers: vec![] });
        Ok(())
    }

    /// A new cycle: a new `boot_id`, `uptime_sec` from zero; called by `run_cycles` before every `start`,
    /// does not touch log lines. The snapshot before the first call carries the id of cycle zero — that is how it was
    /// before the stamp moved out of `output`; do not make the first call conditional.
    pub(crate) fn renew_boot_stamp(&self) {
        let stamp = super::boot_stamp::BootStamp::new();
        self.ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .boot = stamp;
    }

    /// `NoNetwork → Starting`, leaves other phases alone. Called by `run_node_awaiting_key` before parsing
    /// the next `Switch` — "two joins in a row, the first did not parse" must not roll the phase
    /// back to `NoNetwork` in the middle of parsing the second.
    pub(crate) fn mark_starting(&self) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if matches!(st.phase, Phase::NoNetwork) {
            st.phase = Phase::Starting;
        }
    }

    /// `Starting → NoNetwork`, leaves `Stopping` and `Up` alone (if behind a failed `Switch` in the
    /// queue a `Stop` already sits, the "stopping" display must not change to "waiting for a network").
    pub(crate) fn mark_no_network(&self) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if matches!(st.phase, Phase::Starting) {
            st.phase = Phase::NoNetwork;
        }
    }

    /// Action gates: `NoNetwork` → the node is not on a network yet; `Starting`, or `Up` without a ready
    /// snapshot → busy "starting"; `Stopping` → busy "stopping"; otherwise a clone of the node.
    pub(super) fn running_node_for_action(&self) -> Result<Arc<RunningNode>, ActionError> {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match st.phase {
            Phase::NoNetwork => Err(ActionError::Busy(format!(
                "the node has not joined the {0} network yet: paste an invite from a network member or create your own",
                crate::node::output::NET_NAME
            ))),
            Phase::Starting => Err(ActionError::Busy("node is starting".to_string())),
            Phase::Stopping => Err(ActionError::Busy("node is stopping".to_string())),
            Phase::Up => match &st.cache {
                None => Err(ActionError::Busy("node is starting".to_string())),
                Some(_) => Ok(st
                    .node
                    .clone()
                    .expect("Phase::Up always means node = Some")),
            },
        }
    }

    /// Known domain names from the last collection — only for the `check_domain`/`check_all` gates.
    pub(super) fn known_domains(&self) -> Vec<String> {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        st.cache
            .as_ref()
            .map(|c| c.domains.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
