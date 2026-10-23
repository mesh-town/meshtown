//! Page actions: `impl web_gui::NodeControl for NodeCtl` — snapshot, domain search/check,
//! adding a member address, applying settings, joining/leaving a network, invite. State
//! and gates (`running_node_for_action`, `known_domains`) live in `state.rs`; live paths in `live_paths.rs`.

use tokio::sync::mpsc::UnboundedSender;

use crate::config::config_file::Settings;
use crate::invite::InviteHead;
use crate::node::output::{self, say};
use crate::web_gui::{
    ActionError, InviteJoinView, LogPage, MemberActionReq, MemberFuture, NodeControl, NodeSnapshot,
    PendingInvite, SaveKey,
};

use super::invite_wait;
use super::state::{Cmd, NodeCtl, NodeCtlShared, Phase};

impl NodeControl for NodeCtl {
    fn snapshot(&self) -> NodeSnapshot {
        self.snapshot_impl()
    }

    fn log_after(&self, after: u64, limit: usize, net: Option<&str>) -> LogPage {
        output::log_page_for(&self.boot_id(), after, limit.max(1), net)
    }

    fn find_exit(&self, domain: &str) -> Result<(), ActionError> {
        self.find_exit_impl(domain)
    }

    fn check_domain(&self, domain: &str) -> Result<(), ActionError> {
        self.check_domain_impl(domain)
    }

    fn check_all(&self) -> Result<u64, ActionError> {
        self.check_all_impl()
    }

    fn add_domain(&self, domain: &str) -> Result<String, ActionError> {
        self.add_domain_impl(domain)
    }

    fn remove_domain(&self, domain: &str) -> Result<String, ActionError> {
        self.remove_domain_impl(domain)
    }

    fn add_peer_address(&self, line: &str) -> Result<(), String> {
        self.add_peer_address_impl(line)
    }

    fn apply_config(&self, settings: &Settings, paths: &[&'static str]) {
        self.apply_config_impl(settings, paths)
    }

    fn switch_network(&self, key: &str) {
        self.join_with_peers(key, Vec::new());
    }

    fn stop(&self) {
        self.request_stop();
        say("node", "stopping at the page's request");
    }

    fn restart(&self) -> Result<(), ActionError> {
        self.request_restart()?;
        say("node", "restarting the node at the page's request");
        Ok(())
    }

    /// Joining or switching a network: cancels the invite wait and sends the command (`begin_network_switch`).
    fn join_with_peers(&self, key: &str, peers: Vec<crate::transport::net::PeerAddr>) {
        {
            let mut st = self
                .ctl_shared
                .ctl_state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            invite_wait::cancel(&mut st);
        }
        begin_network_switch(&self.ctl_shared, &self.cmd_tx, key, peers);
    }

    fn join_by_invite(
        &self,
        head: InviteHead,
        peers: Vec<crate::transport::net::PeerAddr>,
        save: SaveKey,
    ) -> Result<(), ActionError> {
        invite_wait::start(self, head, peers, save)
    }

    fn members(&self) -> MemberFuture {
        self.members_impl()
    }

    fn member_action(&self, act: MemberActionReq) -> MemberFuture {
        self.member_action_impl(act)
    }

    fn invite_join(&self) -> Option<InviteJoinView> {
        let st = self
            .ctl_shared
            .ctl_state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        invite_wait::view(&st)
    }

    /// Invite of a live node: building the string and recording the issue — in `invite_wait::make_invite`.
    fn invite_pending(&self, for_whom: Option<&str>) -> Result<PendingInvite, ActionError> {
        invite_wait::make_invite(self, for_whom)
    }
}

/// Body of joining a network. From `Stopping` — a refusal, no command is sent. From `NoNetwork`, `Starting`, `Up` —
/// phase `Starting`, command to the channel; the journal line depends on the previous phase (first join or
/// restart) and on whether addresses came with the invite. Does not touch the invite wait:
/// its task calls this function itself.
pub(super) fn begin_network_switch(
    ctl_shared: &NodeCtlShared,
    cmd_tx: &UnboundedSender<Cmd>,
    key: &str,
    peers: Vec<crate::transport::net::PeerAddr>,
) {
    let mut st = ctl_shared
        .ctl_state
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if matches!(st.phase, Phase::Stopping) {
        drop(st);
        say(
            "node",
            format!(
                "stop already in progress, network switch {} skipped",
                crate::node::output::NET_NAME
            ),
        );
        return;
    }
    let from_no_network = matches!(st.phase, Phase::NoNetwork);
    st.phase = Phase::Starting;
    drop(st);
    let peers_count = peers.len();
    let _ = cmd_tx.send(Cmd::Switch {
        key: key.to_string(),
        peers,
    });
    let mut msg = if from_no_network {
        format!(
            "joining the {} network: starting the node",
            crate::node::output::NET_NAME
        )
    } else {
        format!(
            "switching the {} network: restarting the node",
            crate::node::output::NET_NAME
        )
    };
    if peers_count > 0 {
        msg.push_str(&format!(" (addresses from the invite: {peers_count})"));
    }
    say("node", msg);
}
