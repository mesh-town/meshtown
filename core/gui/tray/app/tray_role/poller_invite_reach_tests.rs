//! Tests for the invite reach in `poller_invite`: `reach_from` and a click on "Invite a member"
//! against a stub node with real HTTP. The module is a child of `poller_invite_tests.rs`, so it sees
//! its `SECRET`, `foreign_rig` and `finish`; the click helpers are in `click_support`.

use super::click_support::*;
use super::*;

use serde_json::json;

use crate::gui::tray::app::tray_actions::InviteReach;

// Invite reach: the node puts `reach` in the reply, the poller turns it into `InviteReach`
// and shows it together with the "copied" note. Anything that is not `issuer_only` or `issuer_and_one`
// (an old node without the field, an unknown word, the wrong type, not 200) is `Spread`: no warning.
mod reach_from_reply {
    use super::*;

    mod body_with_reach {
        use super::*;

        #[test]
        fn issuer_only_is_issuer_only() {
            assert_eq!(
                reach_from(&ok(node_says("issuer_only"))),
                InviteReach::IssuerOnly
            );
        }

        #[test]
        fn issuer_and_one_is_issuer_and_one() {
            assert_eq!(
                reach_from(&ok(node_says("issuer_and_one"))),
                InviteReach::IssuerAndOne
            );
        }

        #[test]
        fn spread_is_spread() {
            assert_eq!(reach_from(&ok(node_says("spread"))), InviteReach::Spread);
        }
    }

    mod body_without_usable_reach {
        use super::*;

        #[test]
        fn missing_field_of_old_node_is_spread() {
            assert_eq!(reach_from(&ok(old_node())), InviteReach::Spread);
        }

        #[test]
        fn number_instead_of_string_is_spread() {
            let body = json!({"invite": "abc", "holders": 0, "reach": 5});

            assert_eq!(reach_from(&ok(body)), InviteReach::Spread);
        }

        #[test]
        fn null_is_spread() {
            let body = json!({"invite": "abc", "holders": 0, "reach": null});

            assert_eq!(reach_from(&ok(body)), InviteReach::Spread);
        }

        #[test]
        fn foreign_word_is_spread() {
            assert_eq!(reach_from(&ok(node_says("everyone"))), InviteReach::Spread);
        }
    }

    mod reply_without_body {
        use super::*;

        #[test]
        fn busy_409_is_spread() {
            assert_eq!(
                reach_from(&Ok(Reply::OtherStatus(409))),
                InviteReach::Spread
            );
        }

        #[test]
        fn timeout_is_spread() {
            assert_eq!(reach_from(&Err(ApiError::Timeout)), InviteReach::Spread);
        }

        #[test]
        fn every_other_non_answer_is_spread() {
            let replies = [
                Ok(Reply::OtherStatus(500)),
                Ok(Reply::Unauthorized),
                Ok(Reply::Accepted),
                Ok(Reply::Stopping),
                Err(ApiError::Refused),
                Err(ApiError::NotHttp("not an HTTP reply".to_string())),
            ];

            for reply in &replies {
                assert_eq!(
                    reach_from(reply),
                    InviteReach::Spread,
                    "a reply without a body must not produce a warning"
                );
            }
        }
    }
}

// A click on "Invite a member" against a stub node with real HTTP: what is shown in the menu and what is
// written to the clipboard. The invite string is not put into check messages.
mod click_by_reach {
    use super::*;

    mod node_says_issuer_only {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn menu_gets_copied_issuer_only() {
            let click = setup(node_says("issuer_only"), Clipboard::Takes).await;

            assert_eq!(
                click.notes,
                vec![InviteNote::Copied(InviteReach::IssuerOnly)]
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn invite_string_still_goes_to_clipboard() {
            let click = setup(node_says("issuer_only"), Clipboard::Takes).await;

            assert!(
                click.copied == [SECRET],
                "what went to the clipboard is not the node's invite string"
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn clipboard_refusal_is_plain_failure_without_warning() {
            let click = setup(node_says("issuer_only"), Clipboard::Refuses).await;

            assert_eq!(click.notes, vec![InviteNote::Failed(REASON_CLIP_FAILED)]);
            assert!(
                click.copied.is_empty(),
                "a refusing clipboard accepted nothing"
            );
        }
    }

    mod node_says_issuer_and_one {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn menu_gets_copied_issuer_and_one() {
            let click = setup(node_says("issuer_and_one"), Clipboard::Takes).await;

            assert_eq!(
                click.notes,
                vec![InviteNote::Copied(InviteReach::IssuerAndOne)]
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn clipboard_refusal_is_plain_failure_without_warning() {
            let click = setup(node_says("issuer_and_one"), Clipboard::Refuses).await;

            assert_eq!(click.notes, vec![InviteNote::Failed(REASON_CLIP_FAILED)]);
        }
    }

    mod node_says_spread {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn menu_gets_copied_spread() {
            let click = setup(node_says("spread"), Clipboard::Takes).await;

            assert_eq!(click.notes, vec![InviteNote::Copied(InviteReach::Spread)]);
        }
    }

    mod node_without_usable_reach {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn old_node_without_field_gets_copied_spread() {
            let click = setup(old_node(), Clipboard::Takes).await;

            assert_eq!(click.notes, vec![InviteNote::Copied(InviteReach::Spread)]);
            assert!(
                click.copied == [SECRET],
                "old node: the invite must get copied"
            );
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn foreign_word_gets_copied_spread() {
            let click = setup(node_says("everyone"), Clipboard::Takes).await;

            assert_eq!(click.notes, vec![InviteNote::Copied(InviteReach::Spread)]);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn number_gets_copied_spread() {
            let body = json!({"invite": SECRET, "holders": 0, "reach": 5});

            let click = setup(body, Clipboard::Takes).await;

            assert_eq!(click.notes, vec![InviteNote::Copied(InviteReach::Spread)]);
        }
    }

    mod journal {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn copied_line_is_the_same_for_every_reach() {
            for body in [
                node_says("issuer_only"),
                node_says("issuer_and_one"),
                node_says("spread"),
                old_node(),
            ] {
                let click = setup(body, Clipboard::Takes).await;

                assert_eq!(click.log, vec!["invite: copied".to_string()]);
            }
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn refusal_line_has_no_reach_and_no_invite() {
            let click = setup(node_says("issuer_only"), Clipboard::Refuses).await;

            assert_eq!(
                click.log,
                vec!["invite: couldn't copy — the clipboard didn't accept the text".to_string()]
            );
        }
    }

    // The real route `/api/network/invite` with `FakeNode`: it has no record holders, so
    // the node itself names the reach `issuer_only` — an end-to-end check without a stubbed reply.
    mod real_route_of_fake_node {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn node_without_holders_gives_copied_issuer_only() {
            let (handle, mut rig) = foreign_rig("invite-reach-real", Some(SECRET)).await;

            rig.cmd(NodeCmd::CopyInvite(None));
            let note = rig
                .probe
                .invite("Copied", |n| matches!(n, InviteNote::Copied(_)));

            assert_eq!(note, InviteNote::Copied(InviteReach::IssuerOnly));
            finish(handle, rig).await;
        }
    }
}
