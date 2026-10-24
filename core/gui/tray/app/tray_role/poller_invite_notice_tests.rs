//! Tests for the system notification of a copied invite in `poller_invite`: which reach asks for one, that a failed
//! copy never does, and that the menu line goes first. The module is a child of `poller_invite_tests.rs`;
//! the click helpers are in `click_support`.

use super::click_support::*;
use super::*;

use serde_json::json;

use crate::gui::tray::app::tray_actions::InviteReach;
use crate::gui::tray::app::tray_texts::{
    INVITE_NOTICE_ISSUER_AND_ONE_BODY, INVITE_NOTICE_ISSUER_ONLY_BODY, INVITE_NOTICE_TITLE,
};
use crate::gui::tray::notify::Notice;

mod menu_click {
    use super::*;

    mod copied_with_issuer_only {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn issuer_only_copied_sends_one_notice() {
            let click = setup(node_says("issuer_only"), Clipboard::Takes).await;

            assert_eq!(
                click.notes,
                vec![InviteNote::Copied(InviteReach::IssuerOnly)]
            );
            assert_eq!(click.notices, vec![issuer_only_notice()]);
        }
    }

    mod copied_with_issuer_and_one {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn issuer_and_one_copied_sends_one_notice() {
            let click = setup(node_says("issuer_and_one"), Clipboard::Takes).await;

            assert_eq!(
                click.notes,
                vec![InviteNote::Copied(InviteReach::IssuerAndOne)]
            );
            assert_eq!(click.notices, vec![issuer_and_one_notice()]);
        }
    }

    mod copied_with_spread {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn spread_copied_sends_no_notice() {
            let click = setup(node_says("spread"), Clipboard::Takes).await;

            assert_eq!(click.notes, vec![InviteNote::Copied(InviteReach::Spread)]);
            assert!(click.notices.is_empty(), "spread needs no warning");
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn old_node_sends_no_notice() {
            let click = setup(old_node(), Clipboard::Takes).await;

            assert_eq!(click.notes, vec![InviteNote::Copied(InviteReach::Spread)]);
            assert!(click.notices.is_empty(), "no reach field reads as spread");
        }
    }

    mod clipboard_refuses {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn issuer_only_refused_copy_sends_no_notice() {
            let click = setup(node_says("issuer_only"), Clipboard::Refuses).await;

            assert_eq!(click.notes, vec![InviteNote::Failed(REASON_CLIP_FAILED)]);
            assert!(click.notices.is_empty(), "nothing was copied");
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn issuer_and_one_refused_copy_sends_no_notice() {
            let click = setup(node_says("issuer_and_one"), Clipboard::Refuses).await;

            assert_eq!(click.notes, vec![InviteNote::Failed(REASON_CLIP_FAILED)]);
            assert!(click.notices.is_empty(), "nothing was copied");
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn spread_refused_copy_sends_no_notice() {
            let click = setup(node_says("spread"), Clipboard::Refuses).await;

            assert_eq!(click.notes, vec![InviteNote::Failed(REASON_CLIP_FAILED)]);
            assert!(click.notices.is_empty(), "nothing was copied");
        }
    }

    mod node_gives_no_invite {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn node_error_sends_no_notice() {
            // The reach is risky, but the reply carries no invite string: there is nothing to copy.
            let body = json!({"holders": 0, "reach": "issuer_only"});

            let click = setup(body, Clipboard::Takes).await;

            assert_eq!(click.notes, vec![InviteNote::Failed(REASON_NO_ANSWER)]);
            assert!(click.notices.is_empty(), "no invite, no notice");
        }
    }

    mod order {
        use super::*;

        #[tokio::test(flavor = "multi_thread")]
        async fn menu_line_goes_before_notice() {
            let click = setup(node_says("issuer_only"), Clipboard::Takes).await;

            assert_eq!(click.order, vec!["note", "notice"]);
        }
    }
}

mod network_slot_click {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn slot_invite_issuer_only_sends_notice() {
        let click = setup_slot(node_says("issuer_only"), Clipboard::Takes, "work").await;

        assert_eq!(click.notices, vec![issuer_only_notice()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn slot_invite_spread_sends_no_notice() {
        let click = setup_slot(node_says("spread"), Clipboard::Takes, "work").await;

        assert!(click.notices.is_empty(), "spread needs no warning");
    }
}

// ---- notice helpers ----

fn issuer_only_notice() -> Notice {
    Notice {
        title: INVITE_NOTICE_TITLE,
        body: INVITE_NOTICE_ISSUER_ONLY_BODY,
    }
}

fn issuer_and_one_notice() -> Notice {
    Notice {
        title: INVITE_NOTICE_TITLE,
        body: INVITE_NOTICE_ISSUER_AND_ONE_BODY,
    }
}
