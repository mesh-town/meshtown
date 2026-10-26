//! Tests of the membership-driven tray decisions.

use super::*;
use crate::gui::tray::app::exit_menu::exit_menu_from_snapshot;
use crate::gui::tray::app::snapshot::TraySnapshot;
use serde_json::json;

fn members(admitting: u64, me_excluded: bool) -> TrayMembers {
    TrayMembers {
        admitting,
        me_excluded,
        ..TrayMembers::default()
    }
}

fn asking(admitting: u64, me_pending: bool, me_excluded: bool) -> TrayMembers {
    TrayMembers {
        admitting,
        me_pending,
        me_excluded,
    }
}

#[test]
fn excluded_running_node_gets_the_excluded_line() {
    let line = excluded_line(members(0, true), NodeState::Running);
    assert_eq!(
        line.as_deref(),
        Some("This computer is no longer in the network — sites won't open through Mesh Town")
    );
}

#[test]
fn excluded_line_absent_otherwise() {
    assert_eq!(excluded_line(members(0, false), NodeState::Running), None);
    assert_eq!(excluded_line(members(3, false), NodeState::Running), None);
    assert_eq!(excluded_line(members(0, true), NodeState::Absent), None);
}

#[test]
fn tooltip_has_no_tail_without_requests() {
    assert_eq!(tooltip_text("Mesh Town", members(0, false)), "Mesh Town");
}

#[test]
fn anchor_is_appended_to_plain_and_token_urls() {
    assert_eq!(
        with_anchor("http://127.0.0.1:7070/", "peers"),
        "http://127.0.0.1:7070/#peers"
    );
    assert_eq!(
        with_anchor("http://127.0.0.1:7070/?token=abc", "peers"),
        "http://127.0.0.1:7070/?token=abc#peers"
    );
}

#[test]
fn existing_anchor_is_replaced() {
    assert_eq!(
        with_anchor("http://127.0.0.1:7070/?token=abc#home", "peers"),
        "http://127.0.0.1:7070/?token=abc#peers"
    );
}

#[test]
fn excluded_snapshot_disables_exit_menu_and_carries_members() {
    let snap = TraySnapshot::from_value(&json!({
        "node": {"status": "online", "id_full": "me"},
        "members": {"admitting": 2, "me_excluded": true}
    }));
    let menu = exit_menu_from_snapshot(&snap);
    assert!(!menu.enabled);
    assert_eq!(menu.members, members(2, true));
}

#[test]
fn normal_snapshot_keeps_exit_menu_enabled() {
    let snap = TraySnapshot::from_value(&json!({"node": {"status": "online", "id_full": "me"}}));
    let menu = exit_menu_from_snapshot(&snap);
    assert!(menu.enabled);
    assert_eq!(menu.members, TrayMembers::default());
}

#[test]
fn alert_wanted_with_request_and_not_excluded() {
    assert!(!alert_wanted(members(0, false)));
    assert!(alert_wanted(members(2, false)));
    assert!(!alert_wanted(members(2, true)));
    assert!(alert_wanted(asking(1, false, false)));
    assert!(!alert_wanted(asking(1, false, true)));
    assert!(
        !alert_wanted(asking(0, true, false)),
        "waiting for others needs no action from this computer"
    );
}

#[test]
fn admit_item_shown_only_with_requests() {
    let text = Some("A node is waiting to join — open Members…");
    assert_eq!(admit_item_text(1, false), text);
    assert_eq!(admit_item_text(0, false), None);
    assert_eq!(admit_item_text(1, true), None);
    assert_eq!(members_item_text(asking(2, false, false)), text);
}

#[test]
fn tooltip_tails_for_request_and_pending() {
    assert_eq!(
        tooltip_text("Mesh Town", asking(1, false, false)),
        "Mesh Town · a node is waiting to join"
    );
    assert_eq!(
        tooltip_text("Mesh Town", asking(0, true, false)),
        "Mesh Town · waiting for members to confirm this computer"
    );
    let all = TrayMembers {
        admitting: 1,
        me_pending: true,
        me_excluded: false,
    };
    assert_eq!(
        tooltip_text("Mesh Town", all),
        "Mesh Town · a node is waiting to join · waiting for members to confirm this computer"
    );
}

#[test]
fn tray_members_ignores_the_removed_vote_count() {
    let snap = TraySnapshot::from_value(&json!({"members": {"voting": 2, "me_excluded": false}}));
    assert_eq!(snap.members, members(0, false));
    let new = TraySnapshot::from_value(&json!({"members": {"admitting": 3, "me_pending": true}}));
    assert_eq!(
        new.members,
        TrayMembers {
            admitting: 3,
            me_pending: true,
            ..TrayMembers::default()
        }
    );
}
