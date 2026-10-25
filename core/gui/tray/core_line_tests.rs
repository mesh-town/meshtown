//! Tests of the vote menu item and the icon mark as they follow a snapshot's membership facts.

use super::*;
use crate::gui::tray::app::exit_menu::exit_menu_from_snapshot;
use crate::gui::tray::app::snapshot::TraySnapshot;
use crate::gui::tray::test_tray::TEST_TRAY;
use crate::gui::tray::vote_item::MENU_VOTE;
use serde_json::json;

fn no_link() -> Option<String> {
    None
}

/// A core that has taken the membership facts of a snapshot with `voting` votes.
fn core_after(voting: u64, me_excluded: bool) -> Core {
    let (node_tx, _node_rx) = tokio::sync::mpsc::unbounded_channel();
    let (_tx, mut core) = super::super::channel(
        crate::gui::tray::app::quit::QuitSignal::new(),
        no_link,
        node_tx,
        &TEST_TRAY,
    );
    let snap = TraySnapshot::from_value(&json!({
        "node": {"status": "online", "id_full": "me"},
        "members": {"voting": voting, "me_excluded": me_excluded}
    }));
    core.members = exit_menu_from_snapshot(&snap).members;
    core
}

#[test]
fn vote_item_and_alert_follow_the_vote() {
    let core = core_after(2, false);

    assert_eq!(MENU_VOTE, "vote");
    assert_eq!(
        core.vote_item_text().as_deref(),
        Some("Ban vote in progress — open Members…")
    );
    assert!(core.icon_key().alert);
}

#[test]
fn no_vote_item_without_vote() {
    let core = core_after(0, false);

    assert_eq!(core.vote_item_text(), None);
    assert!(!core.icon_key().alert);
}

#[test]
fn no_vote_item_when_excluded() {
    let core = core_after(2, true);

    assert_eq!(core.vote_item_text(), None);
    assert!(!core.icon_key().alert);
}
