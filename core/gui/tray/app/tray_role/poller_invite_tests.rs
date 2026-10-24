//! Tests for `poller_invite`: parsing the node reply to an invite and a click on "Invite a member" against a
//! real HTTP fake. Time is the real clock, `T.note_visible_for` = 400 ms; the checks are "not earlier than", there are no
//! upper bounds (a 0.25·`note_visible_for` margin covers the probe's event delivery delay).

use super::super::poll_types::PollTiming;
use super::super::poller::test_support::*;
use super::*;

use std::time::{Duration, Instant};

use serde_json::json;

use crate::gui::tray::app::local_api::tests::{start_fake, temp_dir};
use crate::gui::tray::app::tray_actions::InviteNote;
use crate::gui::tray::app::tray_texts::{
    REASON_CLIP_FAILED, REASON_JOINING, REASON_NOT_ADMIN, REASON_NO_ANSWER, REASON_NO_TOOL,
};
use crate::gui::tray::NodeCmd;

const SECRET: &str = "mesh-invite2-SECRETTAILxyz";
/// Lower bound for waiting until the label is restored: `note_visible_for` minus a margin for event delivery delay.
const AT_LEAST: Duration = Duration::from_millis(300);

#[test]
fn invite_from_ok_gives_string() {
    let reply = Ok(Reply::Ok(json!({"invite": "abc"})));
    assert_eq!(invite_from(&reply), Ok("abc".to_string()));
}

#[test]
fn invite_from_409_is_joining() {
    let reply = Ok(Reply::OtherStatus(409));
    assert_eq!(invite_from(&reply), Err(REASON_JOINING));
}

#[test]
fn invite_from_not_admin_gives_its_own_reason() {
    let reply = Ok(Reply::NotAdmin);
    assert_eq!(invite_from(&reply), Err(REASON_NOT_ADMIN));
}

#[test]
fn invite_from_not_admin_is_not_joining() {
    let reply = Ok(Reply::NotAdmin);
    assert_ne!(invite_from(&reply), Err(REASON_JOINING));
}

#[test]
fn not_admin_reason_reads_as_the_failed_note() {
    let note = InviteNote::Failed(REASON_NOT_ADMIN);
    assert_eq!(
        crate::gui::tray::app::tray_actions::invite_item_text(note),
        "Couldn't copy the invite: only admins can invite to this network"
    );
}

#[test]
fn not_admin_does_not_change_the_reach() {
    assert_eq!(reach_from(&Ok(Reply::NotAdmin)), InviteReach::Spread);
}

#[test]
fn invite_from_ok_without_field_is_no_answer() {
    let reply = Ok(Reply::Ok(json!({"other": 1})));
    assert_eq!(invite_from(&reply), Err(REASON_NO_ANSWER));
}

#[test]
fn invite_from_empty_string_is_no_answer() {
    let reply = Ok(Reply::Ok(json!({"invite": ""})));
    assert_eq!(invite_from(&reply), Err(REASON_NO_ANSWER));
}

#[test]
fn invite_from_timeout_is_no_answer() {
    let reply = Err(ApiError::Timeout);
    assert_eq!(invite_from(&reply), Err(REASON_NO_ANSWER));
}

#[test]
fn prod_flash_is_ten_seconds() {
    assert_eq!(PollTiming::PROD.note_visible_for, Duration::from_secs(10));
}

/// A fake node in the `Foreign` phase and a poller pointed at it; the fake's invite is `invite`.
async fn foreign_rig(tag: &str, invite: Option<&str>) -> (crate::web_gui::WebHandle, Rig) {
    let dir = temp_dir(tag);
    let (handle, node) = start_fake(0, &dir).await;
    node.set_invite(invite);
    let port = handle.shared.port();
    let mut rig = Rig::start(port, dir.join("web-token"), true, spawn_panics());
    rig.probe.view("Ready+Running", is_ready);
    (handle, rig)
}

async fn finish(handle: crate::web_gui::WebHandle, mut rig: Rig) {
    rig.quit();
    assert_eq!(rig.code().await, 0);
    handle.shutdown(Duration::from_secs(2)).await;
}

// The "copied" note in these two tests is arbitrary: a fake node without holders names the reach
// `issuer_only`, and the label-restore deadlines do not depend on the reach (the reach is in `poller_invite_reach_tests.rs`).
#[tokio::test(flavor = "multi_thread")]
async fn click_copies_then_title_returns() {
    let (handle, mut rig) = foreign_rig("invite-click", Some(SECRET)).await;

    rig.cmd(NodeCmd::CopyInvite(None));
    rig.probe
        .invite("Copied", |n| matches!(n, InviteNote::Copied(_)));
    let at = Instant::now();
    rig.probe.invite("Idle", |n| *n == InviteNote::Idle);

    assert!(
        at.elapsed() >= AT_LEAST,
        "Idle arrived too early: {:?}",
        at.elapsed()
    );
    assert_eq!(rig.clip.lock().unwrap().copied, vec![SECRET.to_string()]);
    finish(handle, rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn second_click_restarts_timer() {
    let (handle, mut rig) = foreign_rig("invite-twice", Some(SECRET)).await;

    rig.cmd(NodeCmd::CopyInvite(None));
    rig.probe
        .invite("Copied 1", |n| matches!(n, InviteNote::Copied(_)));
    tokio::time::sleep(Duration::from_millis(240)).await;
    rig.cmd(NodeCmd::CopyInvite(None));
    rig.probe
        .invite("Copied 2", |n| matches!(n, InviteNote::Copied(_)));
    let at = Instant::now();
    rig.probe.invite("Idle", |n| *n == InviteNote::Idle);

    assert!(
        at.elapsed() >= AT_LEAST,
        "the deadline was not restarted: {:?}",
        at.elapsed()
    );
    finish(handle, rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn busy_node_shows_joining() {
    let (handle, mut rig) = foreign_rig("invite-busy", None).await;

    rig.cmd(NodeCmd::CopyInvite(None));
    let note = rig
        .probe
        .invite("Failed", |n| matches!(n, InviteNote::Failed(_)));

    assert_eq!(note, InviteNote::Failed(REASON_JOINING));
    assert!(
        rig.clip.lock().unwrap().copied.is_empty(),
        "the stub must not be called"
    );
    finish(handle, rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn clipboard_failure_shows_reason() {
    let (handle, mut rig) = foreign_rig("invite-clipfail", Some(SECRET)).await;
    rig.clip.lock().unwrap().fail = Some(REASON_NO_TOOL);

    rig.cmd(NodeCmd::CopyInvite(None));
    let note = rig
        .probe
        .invite("Failed", |n| matches!(n, InviteNote::Failed(_)));

    assert_eq!(note, InviteNote::Failed(REASON_NO_TOOL));
    finish(handle, rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn secret_never_logged() {
    let (handle, mut rig) = foreign_rig("invite-secret", Some(SECRET)).await;

    rig.cmd(NodeCmd::CopyInvite(None));
    rig.probe.log("invite: copied");
    rig.clip.lock().unwrap().fail = Some(REASON_CLIP_FAILED);
    rig.probe.invite("Idle", |n| *n == InviteNote::Idle);
    rig.cmd(NodeCmd::CopyInvite(None));
    rig.probe
        .log("invite: couldn't copy — the clipboard didn't accept the text");

    rig.probe.drain();
    let logs: String = rig
        .probe
        .history
        .iter()
        .filter_map(|s| match s {
            Seen::Log(l) => Some(l.as_str()),
            Seen::View(_)
            | Seen::Exits(_)
            | Seen::Invite(_)
            | Seen::Serve(_)
            | Seen::StartFail(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!logs.contains("mesh-invite2-"), "invite prefix in the log");
    assert!(!logs.contains("SECRETTAILxyz"), "invite tail in the log");
    finish(handle, rig).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn click_outside_online_phase_is_dropped() {
    let dir = temp_dir("invite-dropped");
    let mut rig = Rig::start(1, dir.join("web-token"), false, spawn_panics());
    let disabled = crate::gui::tray::app::tray_role::views::grey_view(
        crate::gui::tray::app::view::PageState::Disabled,
        crate::gui::tray::app::view::NodeState::Unknown,
    );
    rig.probe.view("Disabled", |v| *v == disabled);

    rig.cmd(NodeCmd::CopyInvite(None));
    tokio::time::sleep(T.note_visible_for * 2).await;
    rig.probe.drain();

    let shown = rig
        .probe
        .history
        .iter()
        .any(|s| matches!(s, Seen::Invite(_)));
    assert!(!shown, "note outside the online phase");
    assert!(
        rig.clip.lock().unwrap().copied.is_empty(),
        "the stub must not be called"
    );
    rig.quit();
    assert_eq!(rig.code().await, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invite_wrong_type_is_no_answer() {
    for body in [
        json!({"invite": 5}),
        json!({"invite": null}),
        json!({"invite": ""}),
    ] {
        let reply = Ok(Reply::Ok(body));
        assert_eq!(invite_from(&reply), Err(REASON_NO_ANSWER));
    }
}

// Invite reach is in a separate file so that this one does not grow past the ceiling.
#[path = "poller_invite_reach_tests.rs"]
mod reach;

// Helpers of a click on the item (stub node, recorded poller outputs) shared by the two files below.
#[path = "poller_invite_click_support.rs"]
mod click_support;

// What the poller does with the system notification after a click.
#[path = "poller_invite_notice_tests.rs"]
mod notice;
