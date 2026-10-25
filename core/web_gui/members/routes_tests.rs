//! Tests of the member-list web API, part one: `GET /api/members`, the node's errors mapped to HTTP, routing
//! (defaults, methods, credentials). Driven through the real router with a fake node (`fixture`).

use axum::http::StatusCode;
use serde_json::json;

use super::fixture::*;
use crate::web_gui::control::ActionError;
use crate::web_gui::members::dto::MembersReply;

mod listing {
    use super::*;

    #[tokio::test]
    async fn get_members_serializes_contract_keys() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        // when
        let (status, body) = fx.get("/api/members").await;
        // then
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, golden(), "body differs from the documented example");
    }

    #[test]
    fn changed_key_is_present_only_when_set() {
        // when
        let plain = serde_json::to_value(sample_reply()).unwrap();
        let noop = serde_json::to_value(MembersReply {
            changed: Some(false),
            ..sample_reply()
        })
        .unwrap();
        // then
        assert!(
            plain.get("changed").is_none(),
            "`changed` must be skipped for a plain list"
        );
        assert_eq!(noop["changed"], json!(false));
    }

    #[tokio::test]
    async fn get_on_busy_node_is_409_busy() {
        // given
        let fx = setup(Answer::Action(ActionError::Busy(
            "Mesh Town is starting".to_string(),
        )));
        // when
        let reply = line(fx.get("/api/members").await);
        // then
        assert_eq!(reply, "409 busy");
    }
}

mod node_errors {
    use super::*;

    #[tokio::test]
    async fn refused_maps_to_409_with_code() {
        // given
        let fx = setup(Answer::Refused {
            code: "self_target",
            message: "use leave for yourself",
        });
        // when
        let (status, body) = fx.post("/api/members/exclude", &id_body()).await;
        // then
        assert_eq!(line((status, body.clone())), "409 self_target");
        assert_eq!(body["message"], "use leave for yourself");
    }

    #[tokio::test]
    async fn busy_maps_to_409_busy() {
        // given
        let fx = setup(Answer::Action(ActionError::Busy(
            "Mesh Town is stopping".to_string(),
        )));
        // when
        let (status, body) = fx.post("/api/members/exclude", &id_body()).await;
        // then
        assert_eq!(line((status, body.clone())), "409 busy");
        assert_eq!(body["message"], "Mesh Town is stopping");
    }

    #[tokio::test]
    async fn other_action_errors_map_to_their_statuses() {
        let cases = [
            (
                ActionError::Invalid("not a node id".to_string()),
                "422 invalid",
            ),
            (
                ActionError::Internal("replica write failed".to_string()),
                "500 internal",
            ),
            (
                ActionError::NotFound("no such node".to_string()),
                "404 not_found",
            ),
        ];
        for (err, want) in cases {
            // given
            let fx = setup(Answer::Action(err));
            // when
            let reply = line(fx.post("/api/members/exclude", &id_body()).await);
            // then
            assert_eq!(reply, want);
        }
    }
}

mod routing {
    use super::*;

    #[tokio::test]
    async fn default_node_control_answers_404() {
        // given: a node that overrides neither `members` nor `member_action`
        let fx = setup_bare();
        // when
        let list = line(fx.get("/api/members").await);
        // then
        assert_eq!(list, "404 not_found", "GET /api/members");
        for action in ACTIONS {
            let reply = line(fx.post_action(action).await);
            assert_eq!(reply, "404 not_found", "POST {action}");
        }
    }

    #[tokio::test]
    async fn wrong_method_is_405() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        let wrong = [
            ("POST", "/api/members"),
            ("DELETE", "/api/members"),
            ("GET", "/api/members/exclude"),
            ("PUT", "/api/members/leave"),
        ];
        for (method, path) in wrong {
            // when
            let reply = line(send(&fx.router, method, path, "", Some(TOKEN)).await);
            // then
            assert_eq!(reply, "405 bad_method", "{method} {path}");
        }
        assert!(fx.seen().is_empty(), "node was called: {:?}", fx.seen());
    }

    #[tokio::test]
    async fn requests_without_token_are_401() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        for (method, path) in [("GET", "/api/members"), ("POST", "/api/members/exclude")] {
            // when
            let reply = line(send(&fx.router, method, path, &id_body(), None).await);
            // then
            assert_eq!(reply, "401 bad_token", "{method} {path}");
        }
    }
}
