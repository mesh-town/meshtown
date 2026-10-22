//! Tests of the invite-join wait on `NodeCtl` without a network: what the page sees, what clears
//! the wait, what it does not change. Joining with a live member (key, order of `save` and network switch, member
//! refusals) — `tests/node_invite_join.rs`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config_file::{Settings, TransportMode};
use crate::invite::{InviteHead, InviteSecret, INVITE_TTL_SECS};
use crate::node_ctl::NodeCtl;
use crate::web::{ActionError, InviteJoinState, InviteJoinView, NodeControl, NodeStatus, SaveKey};

const TEXT_WAITING: &str =
    "Waiting for network members: as soon as anyone from the invite is online, the node will join the network.";
const TEXT_EXPIRED: &str =
    "The invite was issued more than a day ago and is no longer valid. Ask a network member for a new one.";

mod without_invite {
    use super::*;

    #[tokio::test]
    async fn nothing_is_shown() {
        // given
        let fx = setup();

        // then
        assert_eq!(fx.ctl.invite_join(), None);
    }
}

mod fresh_invite_at_a_node_without_network {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shows_waiting_with_the_wait_text() {
        // given
        let fx = setup();

        // when
        fx.join(head(None, now())).unwrap();

        // then
        let view = fx.ctl.invite_join().expect("the wait should be visible");
        assert_eq!(view.state, InviteJoinState::Waiting);
        assert_eq!(view.message, TEXT_WAITING);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shows_the_label_and_the_end_of_the_invite_day() {
        // given
        let fx = setup();
        let invite = head(Some("Alice"), now() - 100);

        // when
        fx.join(invite.clone()).unwrap();

        // then
        let view = fx.ctl.invite_join().unwrap();
        assert_eq!(view.label.as_deref(), Some("Alice"));
        assert_eq!(view.expires_unix, invite.made_unix + INVITE_TTL_SECS);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn node_phase_stays_without_network() {
        // given
        let fx = setup();
        assert_eq!(fx.ctl.snapshot().node.status, NodeStatus::Offline);

        // when
        fx.join(head(None, now())).unwrap();

        // then
        assert_eq!(
            fx.ctl.snapshot().node.status,
            NodeStatus::Offline,
            "waiting for members must not change the node phase"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn key_is_not_written_until_a_participant_answers() {
        // given
        let fx = setup();

        // when
        fx.join(head(None, now())).unwrap();

        // then
        assert_eq!(fx.saved(), Vec::<String>::new());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn view_does_not_go_away_by_reading() {
        // given
        let fx = setup();
        fx.join(head(None, now())).unwrap();

        // when
        let first = fx.ctl.invite_join();
        let second = fx.ctl.invite_join();

        // then
        assert!(first.is_some());
        assert_eq!(first, second);
    }
}

mod second_invite_while_waiting {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn replaces_the_first_one() {
        // given
        let fx = setup();
        fx.join(head(Some("First"), now())).unwrap();

        // when
        fx.join(head(Some("Second"), now())).unwrap();

        // then
        let view = fx.ctl.invite_join().unwrap();
        assert_eq!(view.state, InviteJoinState::Waiting);
        assert_eq!(view.label.as_deref(), Some("Second"));
    }
}

mod waiting_is_cancelled_by {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn switch_network() {
        // given
        let fx = setup_waiting();

        // when
        fx.ctl.switch_network("mesh1-aaaa");

        // then
        assert_eq!(fx.ctl.invite_join(), None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn join_with_peers() {
        // given
        let fx = setup_waiting();

        // when
        fx.ctl.join_with_peers("mesh1-aaaa", vec![]);

        // then
        assert_eq!(fx.ctl.invite_join(), None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stop() {
        // given
        let fx = setup_waiting();

        // when
        fx.ctl.stop();

        // then
        assert_eq!(fx.ctl.invite_join(), None);
        assert_eq!(fx.ctl.snapshot().node.status, NodeStatus::Stopping);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nothing_is_saved_after_the_cancel() {
        // given
        let fx = setup_waiting();

        // when
        fx.ctl.switch_network("mesh1-aaaa");

        // then
        assert_eq!(fx.saved(), Vec::<String>::new());
    }
}

mod node_is_stopping {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn invite_is_refused_as_busy() {
        // given
        let fx = setup();
        fx.ctl.stop();

        // when
        let result = fx.join(head(None, now()));

        // then
        assert_eq!(
            result,
            Err(ActionError::Busy("node is stopping".to_string()))
        );
    }
}

mod expired_invite {
    use super::*;

    #[tokio::test]
    async fn is_invalid_with_the_expired_text() {
        // given
        let fx = setup();
        let expired = head(None, now() - INVITE_TTL_SECS - 10);

        // when
        let result = fx.join(expired);

        // then
        assert_eq!(result, Err(ActionError::Invalid(TEXT_EXPIRED.to_string())));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn leaves_the_waiting_for_an_earlier_invite_alone() {
        // given
        let fx = setup();
        fx.join(head(Some("Live"), now())).unwrap();

        // when
        let result = fx.join(head(Some("Old"), now() - INVITE_TTL_SECS - 10));

        // then
        assert!(matches!(result, Err(ActionError::Invalid(_))));
        let view = fx
            .ctl
            .invite_join()
            .expect("the previous wait should stay");
        assert_eq!(view.state, InviteJoinState::Waiting);
        assert_eq!(view.label.as_deref(), Some("Live"));
    }
}

mod node_without_its_own_key {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn refuses_the_invite_and_saves_nothing() {
        // given: a node whose permanent key is not known to the controller
        let fx = setup_with(None);

        // when: the page starts joining by a fresh invite
        fx.join(head(None, now())).unwrap();

        // then: the wait ends with a refusal (no proof can be signed) and no key is written
        let view = fx.wait_refused().await;
        assert_eq!(view.state, InviteJoinState::Refused);
        assert!(fx.saved().is_empty(), "a key was saved: {:?}", fx.saved());
    }
}

#[path = "invite_wait_late_tests.rs"]
mod last_seconds_of_the_invite_pass;

#[path = "invite_wait_cancel_tests.rs"]
mod key_arrives_after_cancel;

// ============================================================================================
// setup and helpers
// ============================================================================================

struct Fixture {
    ctl: Arc<NodeCtl>,
    saved: Arc<Mutex<Vec<String>>>,
}

impl Fixture {
    /// `join_by_invite` with no members in the string; `save` remembers the key text.
    fn join(&self, invite: InviteHead) -> Result<(), ActionError> {
        self.ctl
            .join_by_invite(invite, vec![], self.recording_save())
    }

    fn recording_save(&self) -> SaveKey {
        let saved = self.saved.clone();
        Box::new(move |text| {
            saved.lock().unwrap().push(text.to_string());
            Ok(())
        })
    }

    /// Texts that `save` was called with.
    fn saved(&self) -> Vec<String> {
        self.saved.lock().unwrap().clone()
    }

    /// Waits up to 10 s for the `Refused` view and returns it.
    async fn wait_refused(&self) -> InviteJoinView {
        for _ in 0..200 {
            if let Some(view) = self.ctl.invite_join() {
                if view.state == InviteJoinState::Refused {
                    return view;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("refusal did not appear within 10 s: {:?}", self.ctl.invite_join());
    }
}

/// A node without a network, without a relay (the invite client does not go outside).
fn setup() -> Fixture {
    setup_with(Some(crate::net::NodeSecret::from_bytes(&[5; 32])))
}

/// The same node; `owner` is its permanent key (the one the invite proof is signed with), if it is known.
fn setup_with(owner: Option<crate::net::NodeSecret>) -> Fixture {
    let settings = Settings {
        net: crate::config_file::NetSettings {
            transport_mode: TransportMode::NoRelay,
            ..crate::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    let ctl = NodeCtl::new_without_network(&settings);
    ctl.set_test_net(Some(crate::net::mem::MemNet::new()));
    if let Some(owner) = owner {
        ctl.set_owner_secret(owner);
    }
    Fixture {
        ctl,
        saved: Arc::new(Mutex::new(Vec::new())),
    }
}

/// `setup` plus an already running wait on a fresh invite.
fn setup_waiting() -> Fixture {
    let fx = setup();
    fx.join(head(Some("Alice"), now())).unwrap();
    assert!(fx.ctl.invite_join().is_some(), "given: the wait should be running");
    fx
}

fn head(label: Option<&str>, made_unix: u64) -> InviteHead {
    InviteHead {
        secret: InviteSecret::generate().unwrap(),
        network_id: [1, 2, 3, 4],
        label: label.map(str::to_string),
        made_unix,
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
