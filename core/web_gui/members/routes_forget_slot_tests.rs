//! The `forget` and `unforget` member routes under `/api/net/{slot}/members`: the id reaches the node of
//! the slot, the main node is not asked, unknown slots are 404 and bad bodies never reach a node.

use axum::http::StatusCode;

use super::forget_support::{id, id_body, setup, ACTIONS, SLOT};
use crate::web_gui::MemberActionReq;

mod slot_prefix {
    use super::*;

    #[tokio::test]
    async fn forget_reaches_the_node_of_the_slot() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w.post(&format!("{SLOT}/members/forget"), &id_body()).await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(w.slot.calls(), vec![MemberActionReq::Forget(id())]);
    }

    #[tokio::test]
    async fn unforget_reaches_the_node_of_the_slot() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w
            .post(&format!("{SLOT}/members/unforget"), &id_body())
            .await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(w.slot.calls(), vec![MemberActionReq::Unforget(id())]);
    }

    #[tokio::test]
    async fn the_main_node_is_not_asked() {
        // given
        let w = setup(None);
        // when
        w.post(&format!("{SLOT}/members/forget"), &id_body()).await;
        // then
        assert!(w.main.calls().is_empty(), "main node: {:?}", w.main.calls());
    }

    #[tokio::test]
    async fn refusals_of_the_slot_node_are_409_with_their_code() {
        for (action, code) in [("forget", "online"), ("unforget", "not_forgotten")] {
            // given
            let w = setup(Some(code));
            // when
            let (status, body) = w
                .post(&format!("{SLOT}/members/{action}"), &id_body())
                .await;
            // then
            assert_eq!(status, StatusCode::CONFLICT, "{action}");
            assert_eq!(body["error"], code, "{action}");
        }
    }

    #[tokio::test]
    async fn unknown_slot_is_404_unknown_net_and_no_node_is_asked() {
        for action in ACTIONS {
            // given
            let w = setup(None);
            // when
            let path = format!("/api/net/zz2c3d/members/{action}");
            let (status, body) = w.post(&path, &id_body()).await;
            // then
            assert_eq!(status, StatusCode::NOT_FOUND, "{action}");
            assert_eq!(body["error"], "unknown_net", "{action}");
            assert!(w.main.calls().is_empty(), "{action}: main node was called");
            assert!(w.slot.calls().is_empty(), "{action}: slot node was called");
        }
    }

    #[tokio::test]
    async fn bad_body_never_reaches_the_node_of_the_slot() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w.post(&format!("{SLOT}/members/forget"), "{}").await;
        // then
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "bad_request");
        assert!(w.slot.calls().is_empty(), "slot node was called");
    }
}
