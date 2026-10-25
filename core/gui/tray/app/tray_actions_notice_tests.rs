//! Tests of the notices raised by the "Invite a member" tray items (`tray_actions.rs`).

use super::*;
use crate::gui::tray::app::tray_texts::{
    INVITE_NOTICE_ISSUER_AND_ONE_BODY, INVITE_NOTICE_ISSUER_ONLY_BODY, INVITE_NOTICE_TITLE,
};
use crate::gui::tray::notify::Notice;

#[test]
fn spread_has_no_notice() {
    assert_eq!(invite_notice(InviteReach::Spread), None);
}

#[test]
fn issuer_and_one_notice_asks_to_keep_on() {
    assert_eq!(
        invite_notice(InviteReach::IssuerAndOne),
        Some(Notice {
            title: INVITE_NOTICE_TITLE,
            body: INVITE_NOTICE_ISSUER_AND_ONE_BODY,
        })
    );
}

#[test]
fn issuer_only_notice_says_only_this_computer() {
    assert_eq!(
        invite_notice(InviteReach::IssuerOnly),
        Some(Notice {
            title: INVITE_NOTICE_TITLE,
            body: INVITE_NOTICE_ISSUER_ONLY_BODY,
        })
    );
}

#[test]
fn notice_title_is_short() {
    for reach in [InviteReach::IssuerOnly, InviteReach::IssuerAndOne] {
        let n = invite_notice(reach).expect("a risky reach has a notice");

        assert!(n.title.chars().count() <= 45, "banner title: {reach:?}");
        assert!(n.body.chars().count() <= 95, "banner body: {reach:?}");
        for text in [n.title, n.body] {
            assert!(!text.contains(['&', '<', '>']), "markup in {text:?}");
            assert!(!text.starts_with('-'), "{text:?} reads as an option");
        }
    }
}

#[test]
fn notice_bodies_lead_with_the_action() {
    for body in [
        INVITE_NOTICE_ISSUER_ONLY_BODY,
        INVITE_NOTICE_ISSUER_AND_ONE_BODY,
    ] {
        assert!(body.starts_with("Keep this computer on"), "{body}");
        let head: String = body.chars().take(55).collect();
        assert!(
            head.contains("Keep") && head.contains("until they join"),
            "{head}"
        );
    }
}
