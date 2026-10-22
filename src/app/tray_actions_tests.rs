//! Tests of the texts of the "Invite a member" tray items (`tray_actions.rs`).

use super::*;
use crate::app::start_note::StartNote;
use crate::app::tray_texts::{
    INVITE_COPIED_ISSUER_AND_ONE_LINE, INVITE_COPIED_ISSUER_ONLY_LINE, REASON_JOINING,
    REASON_NO_TOOL, REASON_SERVE_REFUSED,
};

#[test]
fn idle_text_invites() {
    assert_eq!(invite_item_text(InviteNote::Idle), "Invite a member");
}

#[test]
fn copied_text_has_check() {
    assert_eq!(
        invite_item_text(InviteNote::Copied(InviteReach::Spread)),
        "Invite copied ✓"
    );
}

#[test]
fn failed_text_names_reason() {
    assert_eq!(
        invite_item_text(InviteNote::Failed(REASON_JOINING)),
        "Couldn't copy the invite: Mesh Town is still joining the network"
    );
}

#[test]
fn copied_line_warns_not_to_share() {
    assert_eq!(
        invite_status_line(InviteNote::Copied(InviteReach::Spread)),
        Some(
            "Invite copied — single use, valid for 24 h; send it privately"
                .to_string()
        )
    );
}

#[test]
fn failed_line_names_reason() {
    assert_eq!(
        invite_status_line(InviteNote::Failed(REASON_NO_TOOL)),
        Some(format!(
            "Couldn't copy the invite: {REASON_NO_TOOL}"
        ))
    );
}

#[test]
fn idle_has_no_line() {
    assert_eq!(invite_status_line(InviteNote::Idle), None);
    assert_eq!(serve_status_line(ServeNote::Idle), None);
}

#[test]
fn serve_failed_line_on_and_off() {
    let on = ServeNote::Failed {
        was_enabling: true,
        reason: REASON_SERVE_REFUSED,
    };
    let off = ServeNote::Failed {
        was_enabling: false,
        reason: REASON_SERVE_REFUSED,
    };
    assert_eq!(
        serve_status_line(on),
        Some("Couldn't turn on sharing: Mesh Town didn't accept the change".to_string())
    );
    assert_eq!(
        serve_status_line(off),
        Some("Couldn't turn off sharing: Mesh Town didn't accept the change".to_string())
    );
}

#[test]
fn first_line_prefers_notes_over_status() {
    use crate::app::view::{node_status_menu_line, RouteLine, View};
    let view = View::initial();
    let route = RouteLine::Direct;
    let failed = ServeNote::Failed {
        was_enabling: true,
        reason: REASON_SERVE_REFUSED,
    };

    let plain = menu_first_line(view, &route, InviteNote::Idle, ServeNote::Idle, &StartNote::Idle);
    let copied = InviteNote::Copied(InviteReach::Spread);
    let invite = menu_first_line(view, &route, copied, failed, &StartNote::Idle);
    let serve = menu_first_line(view, &route, InviteNote::Idle, failed, &StartNote::Idle);

    assert_eq!(plain, node_status_menu_line(view, &route));
    assert_eq!(invite, INVITE_COPIED_LINE);
    assert_eq!(serve, "Couldn't turn on sharing: Mesh Town didn't accept the change");
}

#[test]
fn start_note_below_click_notes() {
    use crate::app::start_note::{start_note_line, StartNote};
    let view = View::initial();
    let route = RouteLine::Direct;
    let busy = StartNote::ProxyBusy(1080);

    let copied = InviteNote::Copied(InviteReach::Spread);
    let with_invite = menu_first_line(view, &route, copied, ServeNote::Idle, &busy);
    let alone = menu_first_line(view, &route, InviteNote::Idle, ServeNote::Idle, &busy);

    assert_eq!(with_invite, INVITE_COPIED_LINE);
    assert_eq!(alone, start_note_line(&busy).unwrap());
}

// The reach of an invite: the node reports who will let a newcomer in while the creator is off; the tray
// says so in the first menu line. The texts are literals: a typo in the constant turns the test red.
mod invite_reach {
    use super::*;

    const ISSUER_ONLY_LINE: &str = "Invite copied — only this computer can let them in; keep it on";
    const ISSUER_AND_ONE_LINE: &str =
        "Invite copied — keep this computer on until they join";
    const SPREAD_LINE: &str = "Invite copied — single use, valid for 24 h; send it privately";

    mod from_wire {
        use super::*;

        #[test]
        fn issuer_only_word_is_issuer_only() {
            assert_eq!(
                InviteReach::from_wire(Some("issuer_only")),
                InviteReach::IssuerOnly
            );
        }

        #[test]
        fn issuer_and_one_word_is_issuer_and_one() {
            assert_eq!(
                InviteReach::from_wire(Some("issuer_and_one")),
                InviteReach::IssuerAndOne
            );
        }

        #[test]
        fn spread_word_is_spread() {
            assert_eq!(InviteReach::from_wire(Some("spread")), InviteReach::Spread);
        }

        #[test]
        fn missing_field_of_old_node_is_spread() {
            assert_eq!(InviteReach::from_wire(None), InviteReach::Spread);
        }

        #[test]
        fn foreign_word_is_spread() {
            for word in ["everyone", "", "issuer"] {
                assert_eq!(
                    InviteReach::from_wire(Some(word)),
                    InviteReach::Spread,
                    "an unfamiliar value {word:?} must not give a warning"
                );
            }
        }
    }

    mod copied_line {
        use super::*;

        #[test]
        fn issuer_only_warns_that_only_this_computer_lets_in() {
            let line = invite_status_line(InviteNote::Copied(InviteReach::IssuerOnly));

            assert_eq!(line, Some(ISSUER_ONLY_LINE.to_string()));
        }

        #[test]
        fn issuer_and_one_advises_not_to_switch_off() {
            let line = invite_status_line(InviteNote::Copied(InviteReach::IssuerAndOne));

            assert_eq!(line, Some(ISSUER_AND_ONE_LINE.to_string()));
        }

        #[test]
        fn spread_is_the_old_one_time_line() {
            let line = invite_status_line(InviteNote::Copied(InviteReach::Spread));

            assert_eq!(line, Some(SPREAD_LINE.to_string()));
            assert_eq!(
                SPREAD_LINE, INVITE_COPIED_LINE,
                "the old constant did not change"
            );
        }

        #[test]
        fn constants_hold_the_planned_texts() {
            assert_eq!(INVITE_COPIED_ISSUER_ONLY_LINE, ISSUER_ONLY_LINE);
            assert_eq!(INVITE_COPIED_ISSUER_AND_ONE_LINE, ISSUER_AND_ONE_LINE);
        }

        #[test]
        fn three_lines_differ() {
            let all = [
                invite_status_line(InviteNote::Copied(InviteReach::IssuerOnly)),
                invite_status_line(InviteNote::Copied(InviteReach::IssuerAndOne)),
                invite_status_line(InviteNote::Copied(InviteReach::Spread)),
            ];

            assert_ne!(all[0], all[1]);
            assert_ne!(all[0], all[2]);
            assert_ne!(all[1], all[2]);
        }
    }

    mod item_text {
        use super::*;

        #[test]
        fn label_stays_with_check_for_every_reach() {
            for reach in [
                InviteReach::IssuerOnly,
                InviteReach::IssuerAndOne,
                InviteReach::Spread,
            ] {
                assert_eq!(
                    invite_item_text(InviteNote::Copied(reach)),
                    "Invite copied ✓",
                    "the item label doesn't depend on the reach ({reach:?})"
                );
            }
        }
    }

    mod first_line_of_menu {
        use super::*;

        #[test]
        fn issuer_only_warning_goes_first() {
            let line =
                first_line_with(InviteNote::Copied(InviteReach::IssuerOnly), ServeNote::Idle);

            assert_eq!(line, ISSUER_ONLY_LINE);
        }

        #[test]
        fn issuer_and_one_advice_goes_first() {
            let line = first_line_with(
                InviteNote::Copied(InviteReach::IssuerAndOne),
                ServeNote::Idle,
            );

            assert_eq!(line, ISSUER_AND_ONE_LINE);
        }

        #[test]
        fn warning_beats_serve_failure() {
            let failed = ServeNote::Failed {
                was_enabling: true,
                reason: REASON_SERVE_REFUSED,
            };

            let line = first_line_with(InviteNote::Copied(InviteReach::IssuerOnly), failed);

            assert_eq!(line, ISSUER_ONLY_LINE);
        }

        #[test]
        fn failed_copy_has_no_reach_warning() {
            let line = first_line_with(InviteNote::Failed(REASON_NO_TOOL), ServeNote::Idle);

            assert_eq!(
                line,
                format!("Couldn't copy the invite: {REASON_NO_TOOL}")
            );
        }
    }

    fn first_line_with(invite: InviteNote, serve: ServeNote) -> String {
        let route = RouteLine::Direct;
        menu_first_line(View::initial(), &route, invite, serve, &StartNote::Idle)
    }
}
