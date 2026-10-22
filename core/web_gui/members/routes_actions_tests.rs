//! Tests of the member-list web API, part two: the shape of action bodies and what reaches the node.
//! Driven through the real router with a fake node (`fixture`).

use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use serde_json::json;

use super::fixture::*;
use crate::web_gui::members::dto::MembersReply;

mod action_body {
    use super::*;

    #[tokio::test]
    async fn exclude_without_id_is_400() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        // when
        let reply = line(fx.post("/api/members/exclude", "{}").await);
        // then
        assert_eq!(reply, "400 bad_request");
    }

    #[tokio::test]
    async fn every_id_action_without_id_is_400() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        for action in ["exclude", "unexclude", "block", "unblock"] {
            // when
            let reply = line(
                fx.post(&format!("/api/members/{action}"), r#"{"confirm":true}"#)
                    .await,
            );
            // then
            assert_eq!(reply, "400 bad_request", "{action}");
        }
        assert!(fx.seen().is_empty(), "node was called: {:?}", fx.seen());
    }

    #[tokio::test]
    async fn body_that_is_not_an_object_is_400() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        for text in ["[]", "\"x\"", "null", "42"] {
            // when
            let reply = line(fx.post("/api/members/block", text).await);
            // then
            assert_eq!(reply, "400 bad_request", "body {text}");
        }
    }

    #[tokio::test]
    async fn non_json_body_is_400_bad_request() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        for action in ["exclude", "leave"] {
            for text in ["{not json", "id=abc", "{\"id\":"] {
                // when
                let reply = line(fx.post(&format!("/api/members/{action}"), text).await);
                // then
                assert_eq!(reply, "400 bad_request", "{action} with body {text}");
            }
        }
        assert!(fx.seen().is_empty(), "node was called: {:?}", fx.seen());
    }

    #[tokio::test]
    async fn leave_without_confirm_is_400() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        // when
        let reply = line(fx.post("/api/members/leave", "{}").await);
        // then
        assert_eq!(reply, "400 bad_request");
    }

    #[tokio::test]
    async fn leave_with_confirm_that_is_not_true_is_400() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        for text in [
            r#"{"confirm":false}"#,
            r#"{"confirm":"true"}"#,
            r#"{"confirm":1}"#,
        ] {
            // when
            let reply = line(fx.post("/api/members/leave", text).await);
            // then
            assert_eq!(reply, "400 bad_request", "body {text}");
        }
        assert!(fx.seen().is_empty(), "node was called: {:?}", fx.seen());
    }
}

mod reason {
    use super::*;

    async fn seen_for(body: serde_json::Value) -> (String, Vec<String>) {
        let fx = setup(Answer::List(sample_reply()));
        let reply = line(fx.post("/api/members/exclude", &body.to_string()).await);
        (reply, fx.seen())
    }

    #[tokio::test]
    async fn reason_of_80_characters_reaches_the_node() {
        // given / when: a reason of 80 two-byte characters
        let reason = "é".repeat(80);
        let (reply, seen) = seen_for(json!({"id": target(), "reason": reason})).await;
        // then
        assert_eq!(reply, "200 -");
        assert_eq!(seen, vec![format!("exclude:{}:{reason}", target())]);
    }

    #[tokio::test]
    async fn reason_of_81_characters_is_400() {
        let (reply, seen) = seen_for(json!({"id": target(), "reason": "a".repeat(81)})).await;
        assert_eq!(reply, "400 bad_request");
        assert!(seen.is_empty(), "node was called: {seen:?}");
    }

    #[tokio::test]
    async fn reason_with_a_control_character_is_400() {
        for bad in ["a\nb", "a\tb", "\u{7}", "x\r", "\n", "ok\n"] {
            let (reply, seen) = seen_for(json!({"id": target(), "reason": bad})).await;
            assert_eq!(reply, "400 bad_request", "{bad:?}");
            assert!(seen.is_empty(), "node was called: {seen:?}");
        }
    }

    #[tokio::test]
    async fn reason_that_is_not_a_string_is_400() {
        for bad in [json!(5), json!(true), json!(["a"]), json!({"a": 1})] {
            let (reply, seen) = seen_for(json!({"id": target(), "reason": bad})).await;
            assert_eq!(reply, "400 bad_request", "{bad}");
            assert!(seen.is_empty(), "node was called: {seen:?}");
        }
    }

    #[tokio::test]
    async fn blank_missing_or_null_reason_is_none() {
        for body in [
            json!({"id": target(), "reason": "  "}),
            json!({"id": target()}),
            json!({"id": target(), "reason": null}),
        ] {
            let (reply, seen) = seen_for(body.clone()).await;
            assert_eq!(
                (reply.as_str(), seen),
                ("200 -", vec![format!("exclude:{}", target())]),
                "{body}"
            );
        }
    }

    #[tokio::test]
    async fn reason_is_trimmed_before_it_reaches_the_node() {
        let (_, seen) = seen_for(json!({"id": target(), "reason": "  noisy link "})).await;
        assert_eq!(seen, vec![format!("exclude:{}:noisy link", target())]);
    }

    #[tokio::test]
    async fn reply_vote_carries_the_reason_key() {
        // given / when: the list reply of the fixture
        let fx = setup(Answer::List(sample_reply()));
        let (_, body) = fx.get("/api/members").await;
        // then: the voting row has the key, the others have no vote at all
        assert_eq!(body["members"][1]["vote"]["reason"], json!("noisy link"));
    }
}

mod actions {
    use super::*;

    #[tokio::test]
    async fn exclude_passes_id_to_node() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        // when
        let (status, _) = fx.post("/api/members/exclude", &id_body()).await;
        // then
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fx.seen(), vec![format!("exclude:{}", target())]);
    }

    #[tokio::test]
    async fn all_five_actions_reach_node() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        // when
        for action in ACTIONS {
            let (status, _) = fx.post_action(action).await;
            assert_eq!(status, StatusCode::OK, "{action}");
        }
        // then
        let id = target();
        let want = ACTIONS.map(|a| {
            if a == "leave" {
                a.to_string()
            } else {
                format!("{a}:{id}")
            }
        });
        assert_eq!(
            fx.seen(),
            want,
            "each path must reach the node as its own action, in order"
        );
    }

    #[tokio::test]
    async fn reply_is_the_list_with_the_changed_flag_from_node() {
        for changed in [true, false] {
            // given
            let fx = setup(Answer::List(MembersReply {
                changed: Some(changed),
                ..sample_reply()
            }));
            // when
            let (status, body) = fx.post("/api/members/block", &id_body()).await;
            // then
            let mut want = golden();
            want["changed"] = json!(changed);
            assert_eq!(
                (status, body),
                (StatusCode::OK, want),
                "changed = {changed}"
            );
        }
    }

    #[tokio::test]
    async fn action_while_node_is_stopping_is_503() {
        // given
        let fx = setup(Answer::List(sample_reply()));
        fx.app.stopping.store(true, Ordering::SeqCst);
        // when
        let reply = line(fx.post("/api/members/exclude", &id_body()).await);
        // then
        assert_eq!(reply, "503 stopping");
        assert!(fx.seen().is_empty(), "node was called: {:?}", fx.seen());
    }
}
