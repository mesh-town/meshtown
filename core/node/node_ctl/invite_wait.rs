//! Joining by invite on a node: waiting for members in a background task, and the state for the page.
//!
//! What it does: `start` checks the string's expiry, cancels the previous wait and starts `redeem`;
//! on receiving the key, saves it (`SaveKey`) and sends the node a join-network command; a final refusal
//! leaves the `Refused` view in the state until a new join, network creation or a stop clears it.
//! What it does not do: does not change the node phase during the wait and does not parse the invite string.
//! Links: `control.rs` (`NodeControl` methods), `state.rs` (fields `invite_wait`, `invite_gen`).

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc::UnboundedSender;
use tokio::task::AbortHandle;

use super::control::begin_network_switch;
use super::state::{Cmd, NodeCtl, NodeCtlShared, NodeCtlState, Phase};
use crate::invite::redeem::{redeem, RedeemError, RedeemOpts};
use crate::invite::{check_fresh, InviteHead, NotAdmin, INVITE_TTL_SECS};
use crate::node::output::say;
use crate::transport::net::{NodeSecret, PeerAddr};
use crate::web_gui::{ActionError, InviteJoinState, InviteJoinView, PendingInvite, SaveKey};

const TEXT_WAITING: &str =
    "Waiting for network members: as soon as anyone from the invite is online, \
the node will join the network.";

/// Slot in `NodeCtlState`: the generation the page sees, and the task handle (`Refused` no longer has one).
pub(super) struct InviteWait {
    invite_gen: u64,
    view: InviteJoinView,
    task: Option<AbortHandle>,
}

/// Invite of a live node. Under the lock — only choosing and cloning the node; the string is built without
/// the lock, recording the issue in the network document — in the returned `PendingInvite`.
pub(super) fn make_invite(
    ctl: &NodeCtl,
    for_whom: Option<&str>,
) -> Result<PendingInvite, ActionError> {
    let node = {
        let st = ctl
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match (&st.phase, &st.node) {
            (Phase::Up, Some(n)) => n.clone(),
            (Phase::NoNetwork, _) => {
                return Err(ActionError::Busy(format!(
                    "the node has not joined the {} network yet",
                    crate::node::output::NET_NAME
                )))
            }
            (Phase::Stopping, _) => return Err(ActionError::Busy("node is stopping".to_string())),
            _ => return Err(ActionError::Busy("node is starting".to_string())),
        }
    };
    if node.stack.is_closed() {
        return Err(ActionError::Busy("node is stopping".to_string()));
    }
    let issued = crate::invite::make(&node.invite_source(), for_whom).map_err(make_error)?;
    Ok(PendingInvite::pending(issued.commit()))
}

/// The page-facing error of a failed `invite::make`: the admin-only refusal has its own code.
fn make_error(e: anyhow::Error) -> ActionError {
    if let Some(denied) = e.downcast_ref::<NotAdmin>() {
        return ActionError::Denied {
            code: "not_admin",
            message: denied.to_string(),
        };
    }
    ActionError::Internal(format!("invite could not be built: {e:#}"))
}

/// Starts the wait. A previous wait or refusal is cleared; the node phase does not change.
pub(super) fn start(
    ctl: &NodeCtl,
    head: InviteHead,
    peers: Vec<PeerAddr>,
    save: SaveKey,
) -> Result<(), ActionError> {
    let mut st = ctl
        .ctl_shared
        .ctl_state
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if matches!(st.phase, Phase::Stopping) {
        return Err(ActionError::Busy("node is stopping".to_string()));
    }
    check_fresh(&head, now_unix()).map_err(|e| ActionError::Invalid(e.to_string()))?;
    cancel(&mut st);
    st.invite_gen += 1;
    let invite_gen = st.invite_gen;
    let view = waiting_view(&head);
    let test_net = st.test_net.clone();
    let owner = st.owner.clone();
    let job = Job {
        ctl_shared: ctl.ctl_shared.clone(),
        cmd_tx: ctl.cmd_tx.clone(),
        invite_gen,
        head,
        peers,
        save,
        test_net,
        owner,
    };
    let task = crate::node::output::spawn_on(&ctl.rt, job.run());
    st.invite_wait = Some(InviteWait {
        invite_gen,
        view,
        task: Some(task.abort_handle()),
    });
    drop(st);
    say("node", "joining by invite: waiting for network members");
    Ok(())
}

/// Clears the wait or refusal: the task is aborted, the slot is emptied.
pub(super) fn cancel(st: &mut NodeCtlState) {
    let Some(wait) = st.invite_wait.take() else {
        return;
    };
    if let Some(task) = wait.task {
        task.abort();
    }
}

/// What to show the page.
pub(super) fn view(st: &NodeCtlState) -> Option<InviteJoinView> {
    st.invite_wait.as_ref().map(|w| w.view.clone())
}

fn waiting_view(head: &InviteHead) -> InviteJoinView {
    InviteJoinView {
        state: InviteJoinState::Waiting,
        message: TEXT_WAITING.to_string(),
        label: head.label.clone(),
        expires_unix: head.made_unix.saturating_add(INVITE_TTL_SECS),
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Everything the wait task needs; `run` lives in the node runtime.
struct Job {
    ctl_shared: Arc<NodeCtlShared>,
    cmd_tx: UnboundedSender<Cmd>,
    invite_gen: u64,
    head: InviteHead,
    peers: Vec<PeerAddr>,
    save: SaveKey,
    test_net: Option<Arc<crate::transport::net::mem::MemNet>>,
    owner: Option<NodeSecret>,
}

impl Job {
    async fn run(self) {
        let Some(owner) = self.owner.clone() else {
            return self.refuse(RedeemError::Bind(
                "this computer's own key is not known".to_string(),
            ));
        };
        let opts = RedeemOpts::for_head(&self.head);
        let transport = match crate::node::bind_ephemeral(self.test_net.as_ref()).await {
            Ok(t) => t,
            Err(e) => return self.refuse(RedeemError::Bind(format!("{e:#}"))),
        };
        let result = redeem(&self.head, &self.peers, &opts, &|_| {}, transport, &owner).await;
        match result {
            Ok(key) => {
                let text = self.head.key_text(&key);
                self.accept(text)
            }
            Err(why) => self.refuse(why),
        }
    }

    /// Key received: clearing the slot if the generation matches (a cancelled wait writes
    /// nothing), writing to the file (if that fails — a journal line, the node still joins), joining the network
    /// with the addresses from the string.
    fn accept(self, key_text: String) {
        if !release(&self.ctl_shared, self.invite_gen) {
            return;
        }
        if let Err(e) = (self.save)(&key_text) {
            say(
                "node",
                format!("network key was not saved to the settings file: {e}"),
            );
        }
        begin_network_switch(&self.ctl_shared, &self.cmd_tx, &key_text, self.peers);
    }

    /// Final refusal: if the generation matches, the slot becomes `Refused` without a task.
    fn refuse(self, why: RedeemError) {
        let mut st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let Some(wait) = st
            .invite_wait
            .as_mut()
            .filter(|w| w.invite_gen == self.invite_gen)
        else {
            return;
        };
        wait.view.state = InviteJoinState::Refused;
        wait.view.message = why.to_string();
        wait.task = None;
        drop(st);
        say("node", format!("joining by invite was rejected: {why}"));
    }
}

/// Clears the slot of this generation; `false` — it has already been replaced or cancelled.
fn release(ctl_shared: &NodeCtlShared, invite_gen: u64) -> bool {
    let mut st = ctl_shared
        .ctl_state
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mine = st
        .invite_wait
        .as_ref()
        .is_some_and(|w| w.invite_gen == invite_gen);
    if mine {
        st.invite_wait = None;
    }
    mine
}

#[cfg(test)]
#[path = "invite_wait_tests.rs"]
mod tests;
