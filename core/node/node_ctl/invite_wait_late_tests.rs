//! Tests of the invite wait: the last seconds of the expiry and a member's refusal (split from `invite_wait_tests.rs`).

use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refusal_stays_in_view_with_the_error_text() {
    // given: three seconds remain until the invite's day ends, no members in the string
    let fx = setup();
    let invite = head(Some("Alice"), now() - INVITE_TTL_SECS + 3);

    // when
    fx.join(invite.clone()).unwrap();
    let refused = fx.wait_refused().await;

    // then
    assert_eq!(refused.message, TEXT_EXPIRED);
    assert_eq!(refused.label.as_deref(), Some("Alice"));
    assert_eq!(refused.expires_unix, invite.made_unix + INVITE_TTL_SECS);
    assert_eq!(fx.saved(), Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refusal_does_not_go_away_by_itself_or_by_reading() {
    // given
    let fx = setup();
    fx.join(head(None, now() - INVITE_TTL_SECS + 3)).unwrap();
    let refused = fx.wait_refused().await;

    // when
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = fx.ctl.invite_join();

    // then
    assert_eq!(fx.ctl.invite_join(), Some(refused));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refusal_keeps_the_node_phase() {
    // given
    let fx = setup();
    fx.join(head(None, now() - INVITE_TTL_SECS + 3)).unwrap();

    // when
    fx.wait_refused().await;

    // then
    assert_eq!(fx.ctl.snapshot().node.status, NodeStatus::Offline);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn next_invite_replaces_the_refusal() {
    // given
    let fx = setup();
    fx.join(head(None, now() - INVITE_TTL_SECS + 3)).unwrap();
    fx.wait_refused().await;

    // when
    fx.join(head(Some("New"), now())).unwrap();

    // then
    let view = fx.ctl.invite_join().unwrap();
    assert_eq!(view.state, InviteJoinState::Waiting);
    assert_eq!(view.label.as_deref(), Some("New"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn switch_network_clears_the_refusal() {
    // given
    let fx = setup();
    fx.join(head(None, now() - INVITE_TTL_SECS + 3)).unwrap();
    fx.wait_refused().await;

    // when
    fx.ctl.switch_network("mesh1-aaaa");

    // then
    assert_eq!(fx.ctl.invite_join(), None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_owner_secret_refuses_without_network() {
    // given: a node whose permanent key is not known, with no network at all
    let fx = setup_with(None);

    // when
    fx.join(head(Some("Alice"), now())).unwrap();
    let refused = fx.wait_refused().await;

    // then: the wait is refused up front and no key is saved
    assert!(
        refused
            .message
            .contains("this computer's own key is not known"),
        "{}",
        refused.message
    );
    assert!(!refused.message.contains("node"), "{}", refused.message);
    assert_eq!(fx.saved(), Vec::<String>::new());
}
