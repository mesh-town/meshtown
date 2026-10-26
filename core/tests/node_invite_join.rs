//! A node without a network joining by an invite via `NodeCtl::join_by_invite` with a live member on loopback:
//! the member's key is written to `save` before the network change, the node moves on to starting, the member's refusals are seen
//! as `Refused` and don't touch the node.
//! What it doesn't do: it doesn't check waiting without members, cancellation and shutdown (the unit tests
//! `node_ctl/invite_wait_tests.rs`), the page and the settings file (`core/tests/node_web_bootstrap.rs`).
//! Links: `meshtown_core::node::node_ctl::NodeCtl`, the node harness is `core/tests/common/invite_nodes.rs`.

#[path = "common/invite_nodes.rs"]
mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meshtown_core::config::config_file::Settings;
use meshtown_core::invite::InviteHead;
use meshtown_core::members::network_key::{parse_user_input, NetworkKey};
use meshtown_core::node::node_ctl::NodeCtl;
use meshtown_core::transport::net_mainline::unix_now;
use meshtown_core::web_gui::{InviteJoinState, InviteJoinView, NodeControl, NodeStatus, SaveKey};

use common::{fresh_head, record_issue, setup, wait_until, Rig, ENTRY_LIMIT_SECS};

const TEXT_USED: &str = "Another computer has already joined with this invite — it is single-use. Ask whoever sent it for a new one.";
const TEXT_WRONG_NETWORK: &str =
    "A member answered with the key of another network — the invite was not accepted.";

mod fresh_invite_with_the_issuer_online {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn key_of_the_issuer_goes_to_save_with_the_label() {
        // given
        let rig = setup().await;
        let head = rig.issue_labeled("Masha").await;
        let newcomer = newcomer();

        // when
        newcomer.join(&head, &rig);
        newcomer.wait_started().await;

        // then
        let saved = newcomer.saved();
        assert_eq!(saved.len(), 1, "save must be called exactly once");
        let parsed = parse_user_input(&saved[0].text).expect("a parseable key went to save");
        assert_eq!(
            parsed.base,
            rig.key.to_bytes(),
            "the member's key, not a foreign one"
        );
        assert!(
            saved[0].text.starts_with("Masha--mesh2-"),
            "the label from the string must come before the key, got the start {:?}",
            saved[0].text.chars().take(14).collect::<String>()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn save_is_called_before_the_network_switches() {
        // given
        let rig = setup().await;
        let head = rig.issue_labeled("Masha").await;
        let newcomer = newcomer();

        // when
        newcomer.join(&head, &rig);
        newcomer.wait_started().await;

        // then
        assert_eq!(
            newcomer.saved()[0].status_then,
            NodeStatus::Offline,
            "at the time of save the node is still without a network: joining the network happens after the key is written"
        );
        assert_eq!(newcomer.status(), NodeStatus::Starting);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn waiting_view_goes_away_after_entering() {
        // given
        let rig = setup().await;
        let head = rig.issue_labeled("Masha").await;
        let newcomer = newcomer();

        // when
        newcomer.join(&head, &rig);
        newcomer.wait_started().await;

        // then
        assert_eq!(newcomer.ctl.invite_join(), None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn node_enters_even_if_save_fails() {
        // given
        let rig = setup().await;
        let head = rig.issue_labeled("Masha").await;
        let newcomer = newcomer_with_broken_save();

        // when
        newcomer.join(&head, &rig);
        newcomer.wait_started().await;

        // then
        assert_eq!(newcomer.saved().len(), 1, "save was called all the same");
        assert_eq!(newcomer.ctl.invite_join(), None);
    }
}

mod invite_already_used_by_another_node {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_refused_with_the_used_text() {
        // given
        let (rig, head) = used_invite().await;
        let second = newcomer();

        // when
        second.join(&head, &rig);
        let refused = second.wait_refused().await;

        // then
        assert_eq!(refused.message, TEXT_USED);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn key_is_not_saved_and_the_node_stays_without_network() {
        // given
        let (rig, head) = used_invite().await;
        let second = newcomer();

        // when
        second.join(&head, &rig);
        second.wait_refused().await;

        // then
        assert!(
            second.saved().is_empty(),
            "the key is not written for a redeemed invite"
        );
        assert_eq!(second.status(), NodeStatus::Offline);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn refusal_stays_in_view_until_the_next_action() {
        // given
        let (rig, head) = used_invite().await;
        let second = newcomer();
        second.join(&head, &rig);
        let refused = second.wait_refused().await;

        // when
        tokio::time::sleep(Duration::from_millis(300)).await;

        // then
        assert_eq!(second.ctl.invite_join(), Some(refused));

        // when
        second.ctl.switch_network("mesh2-aaaa");

        // then
        assert_eq!(second.ctl.invite_join(), None);
    }
}

mod invite_with_the_key_of_another_network {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_refused_with_the_wrong_network_text() {
        // given: the member knows the secret, but the network in the string is different
        let rig = setup().await;
        let head = foreign_network_invite(&rig).await;
        let newcomer = newcomer();

        // when
        newcomer.join(&head, &rig);
        let refused = newcomer.wait_refused().await;

        // then
        assert_eq!(refused.message, TEXT_WRONG_NETWORK);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn foreign_key_is_not_saved() {
        // given
        let rig = setup().await;
        let head = foreign_network_invite(&rig).await;
        let newcomer = newcomer();

        // when
        newcomer.join(&head, &rig);
        newcomer.wait_refused().await;

        // then
        assert!(
            newcomer.saved().is_empty(),
            "a key of a different network is not written"
        );
        assert_eq!(newcomer.status(), NodeStatus::Offline);
    }
}

// ============================================================================================
// setup and helpers
// ============================================================================================

/// What `save` saw: the key text and the node phase at that moment.
#[derive(Clone)]
struct Saved {
    text: String,
    status_then: NodeStatus,
}

/// A newcomer: a node without a network and a record of its `save` calls.
struct Newcomer {
    ctl: Arc<NodeCtl>,
    saves: Arc<Mutex<Vec<Saved>>>,
    save_result: Result<(), String>,
}

impl Newcomer {
    /// Joining by an invite through the member `rig`.
    fn join(&self, head: &InviteHead, rig: &Rig) {
        let save = self.recording_save();
        self.ctl
            .join_by_invite(head.clone(), vec![rig.addr.clone()], save)
            .expect("join_by_invite must accept a fresh invite");
    }

    fn recording_save(&self) -> SaveKey {
        let ctl = self.ctl.clone();
        let saves = self.saves.clone();
        let result = self.save_result.clone();
        Box::new(move |text| {
            let status_then = ctl.snapshot().node.status;
            saves.lock().unwrap().push(Saved {
                text: text.to_string(),
                status_then,
            });
            result
        })
    }

    fn saved(&self) -> Vec<Saved> {
        self.saves.lock().unwrap().clone()
    }

    fn status(&self) -> NodeStatus {
        self.ctl.snapshot().node.status
    }

    /// Waits until the node moves on to starting (the join-the-network command has been sent).
    async fn wait_started(&self) {
        let started = wait_until(Duration::from_secs(ENTRY_LIMIT_SECS), || {
            self.status() == NodeStatus::Starting
        })
        .await;
        assert!(
            started,
            "the node did not move on to starting within {ENTRY_LIMIT_SECS} s"
        );
    }

    /// Waits for the `Refused` view and returns it.
    async fn wait_refused(&self) -> InviteJoinView {
        let refused = wait_until(Duration::from_secs(ENTRY_LIMIT_SECS), || {
            self.ctl
                .invite_join()
                .is_some_and(|v| v.state == InviteJoinState::Refused)
        })
        .await;
        assert!(
            refused,
            "the refusal did not appear within {ENTRY_LIMIT_SECS} s: {:?}",
            self.ctl.invite_join()
        );
        self.ctl.invite_join().unwrap()
    }
}

impl Rig {
    /// An invite issued by the member, with a label in the string.
    async fn issue_labeled(&self, label: &str) -> InviteHead {
        let mut head = self.issue(unix_now()).await;
        head.label = Some(label.to_string());
        head
    }
}

fn newcomer() -> Newcomer {
    newcomer_saving(Ok(()))
}

fn newcomer_with_broken_save() -> Newcomer {
    newcomer_saving(Err("read-only disk".to_string()))
}

fn newcomer_saving(save_result: Result<(), String>) -> Newcomer {
    let settings = Settings::default();
    let ctl = NodeCtl::new_without_network(&settings);
    ctl.set_test_net(Some(meshtown_core::transport::net::mem::process_net()));
    ctl.set_owner_secret(meshtown_core::transport::net::NodeSecret::from_bytes(
        &[5; 32],
    ));
    Newcomer {
        ctl,
        saves: Arc::new(Mutex::new(Vec::new())),
        save_result,
    }
}

/// An invite that the first node has already joined with.
async fn used_invite() -> (Rig, InviteHead) {
    let rig = setup().await;
    let head = rig.issue_labeled("Masha").await;
    let first = newcomer();
    first.join(&head, &rig);
    first.wait_started().await;
    (rig, head)
}

/// The secret was issued by the member `rig`, while `network_id` in the string is from a different network.
async fn foreign_network_invite(rig: &Rig) -> InviteHead {
    let other = NetworkKey::generate().unwrap();
    let head = fresh_head(&other, unix_now());
    record_issue(&rig.a, &head, unix_now()).await;
    assert_ne!(
        head.network_id,
        rig.key.subkeys().network_id,
        "given: the network in the string is foreign"
    );
    head
}
