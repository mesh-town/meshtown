//! The `forget` and `unforget` member routes on the main network (`/api/members`): the id reaches the node
//! as the right action, the node's refusals come back as 409 with their code, bad bodies never reach it.

use axum::http::{Method, StatusCode};
use serde_json::json;

use super::forget_support::{id, id_body, setup, ACTIONS, MESSAGE};
use crate::web_gui::MemberActionReq;

mod main_network {
    use super::*;

    #[tokio::test]
    async fn forget_passes_the_id_to_the_node_as_forget() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w.post("/api/members/forget", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(w.main.calls(), vec![MemberActionReq::Forget(id())]);
    }

    #[tokio::test]
    async fn unforget_passes_the_id_to_the_node_as_unforget() {
        // given
        let w = setup(None);
        // when
        let (status, body) = w.post("/api/members/unforget", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(w.main.calls(), vec![MemberActionReq::Unforget(id())]);
    }

    #[tokio::test]
    async fn the_id_is_trimmed_before_it_reaches_the_node() {
        // given
        let w = setup(None);
        // when
        let body = json!({"id": format!("  {}\n", id())}).to_string();
        let (status, _) = w.post("/api/members/forget", &body).await;
        // then
        assert_eq!(status, StatusCode::OK);
        assert_eq!(w.main.calls(), vec![MemberActionReq::Forget(id())]);
    }

    #[tokio::test]
    async fn reply_is_the_member_list_of_the_node() {
        // given
        let w = setup(None);
        // when
        let (_, body) = w.post("/api/members/forget", &id_body()).await;
        // then
        assert_eq!(body["me"], "me");
        assert_eq!(body["counts"]["forgotten"], 0);
        assert_eq!(body["me_removed"], false);
    }

    #[tokio::test]
    async fn only_the_main_node_is_asked() {
        // given
        let w = setup(None);
        // when
        w.post("/api/members/forget", &id_body()).await;
        // then
        assert!(w.slot.calls().is_empty(), "slot node: {:?}", w.slot.calls());
    }
}

mod node_refuses {
    use super::*;

    #[tokio::test]
    async fn forget_of_an_online_member_is_409_online() {
        // given
        let w = setup(Some("online"));
        // when
        let (status, body) = w.post("/api/members/forget", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "online");
        assert_eq!(body["message"], MESSAGE);
    }

    #[tokio::test]
    async fn unforget_of_a_member_not_forgotten_is_409_not_forgotten() {
        // given
        let w = setup(Some("not_forgotten"));
        // when
        let (status, body) = w.post("/api/members/unforget", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "not_forgotten");
        assert_eq!(body["message"], MESSAGE);
    }
}

mod bad_request {
    use super::*;

    #[tokio::test]
    async fn body_without_id_is_400() {
        for action in ACTIONS {
            // given
            let w = setup(None);
            // when
            let (status, body) = w
                .post(&format!("/api/members/{action}"), r#"{"confirm":true}"#)
                .await;
            // then
            assert_eq!(status, StatusCode::BAD_REQUEST, "{action}");
            assert_eq!(body["error"], "bad_request", "{action}");
            assert!(w.main.calls().is_empty(), "{action}: node was called");
        }
    }

    #[tokio::test]
    async fn blank_or_non_string_id_is_400() {
        for bad in [json!(""), json!("   "), json!(5), json!(null), json!(["a"])] {
            for action in ACTIONS {
                // given
                let w = setup(None);
                // when
                let body = json!({ "id": bad }).to_string();
                let (status, reply) = w.post(&format!("/api/members/{action}"), &body).await;
                // then
                assert_eq!(status, StatusCode::BAD_REQUEST, "{action} with id {bad}");
                assert_eq!(reply["error"], "bad_request", "{action} with id {bad}");
                assert!(w.main.calls().is_empty(), "{action} with id {bad}");
            }
        }
    }

    #[tokio::test]
    async fn body_that_is_not_a_json_object_is_400() {
        for text in ["[]", "\"x\"", "null", "42", "{not json", "id=abc", ""] {
            for action in ACTIONS {
                // given
                let w = setup(None);
                // when
                let (status, reply) = w.post(&format!("/api/members/{action}"), text).await;
                // then
                assert_eq!(
                    status,
                    StatusCode::BAD_REQUEST,
                    "{action} with body {text:?}"
                );
                assert_eq!(reply["error"], "bad_request", "{action} with body {text:?}");
                assert!(w.main.calls().is_empty(), "{action} with body {text:?}");
            }
        }
    }

    #[tokio::test]
    async fn get_is_405_bad_method() {
        for action in ACTIONS {
            // given
            let w = setup(None);
            // when
            let (status, body) = w
                .request(Method::GET, &format!("/api/members/{action}"))
                .await;
            // then
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{action}");
            assert_eq!(body["error"], "bad_method", "{action}");
            assert!(w.main.calls().is_empty(), "{action}: node was called");
        }
    }
}
