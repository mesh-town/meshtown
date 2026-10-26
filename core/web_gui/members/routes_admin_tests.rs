//! Tests of the member-list web API for administered networks: `members/admin`, `network/delete`
//! and the fields of the member list that tell its administrators. Driven through the real router with a fake node (`fixture`).

// assumed: this file is declared in `routes.rs` as
// `#[cfg(test)] #[path = "routes_admin_tests.rs"] mod admin_tests;`.
// helper: `routes_fixture_tests.rs` needs the arms `MemberActionReq::Grant(id) => "grant:<id>"`,
// `Revoke(id) => "revoke:<id>"` and `Close => "close"` in `FakeNode::member_action`, the new fields of
// `MembersReply` (`admins: vec![]`, `me_admin: false`) in `sample_reply`, and `admin: false`
// in `row`. The fake node answers canned replies, so the administrator rules themselves are tested at
// `Access` (`actions_managed_tests.rs`); here the route, the body checks and the error mapping are.

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::fixture::*;
use crate::web_gui::members::dto::MembersReply;

fn not_admin() -> Answer {
    Answer::Refused {
        code: "not_admin",
        message: "only admins can do this in this network",
    }
}

fn refused(code: &'static str) -> Answer {
    Answer::Refused {
        code,
        message: "refused by the node",
    }
}

mod make_admin {
    use super::*;

    #[tokio::test]
    async fn grants_the_role_and_returns_the_list_with_the_new_administrator() {
        // given: a managed network where the node accepts the grant
        let fx = setup(Answer::List(managed_reply(&[target()])));
        // when: an administrator makes the node an administrator
        let (status, body) = fx.post("/api/members/admin", &admin_body(true)).await;
        // then: the node was asked to grant, and the reply lists the administrator
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(fx.seen(), vec![format!("grant:{}", target())]);
        assert!(
            body["admins"]
                .as_array()
                .is_some_and(|a| a.contains(&json!(target()))),
            "admins must contain the id: {body}"
        );
    }

    #[tokio::test]
    async fn takes_the_role_away() {
        // given
        let fx = setup(Answer::List(managed_reply(&[])));
        // when
        let reply = line(fx.post("/api/members/admin", &admin_body(false)).await);
        // then
        assert_eq!(reply, "200 -");
        assert_eq!(fx.seen(), vec![format!("revoke:{}", target())]);
    }

    #[tokio::test]
    async fn a_member_without_the_role_is_refused_as_not_admin() {
        let fx = setup(not_admin());
        let reply = line(fx.post("/api/members/admin", &admin_body(true)).await);
        assert_eq!(reply, "409 not_admin");
    }

    #[tokio::test]
    async fn the_refusal_carries_the_message_of_the_node() {
        let fx = setup(not_admin());
        let (_, body) = fx.post("/api/members/admin", &admin_body(true)).await;
        assert_eq!(body["message"], "only admins can do this in this network");
    }

    #[tokio::test]
    async fn the_last_administrator_cannot_take_the_role_off() {
        let fx = setup(refused("last_admin"));
        let reply = line(fx.post("/api/members/admin", &admin_body(false)).await);
        assert_eq!(reply, "409 last_admin");
    }

    #[tokio::test]
    async fn a_node_that_is_not_a_member_is_refused_as_not_member() {
        let fx = setup(refused("not_member"));
        let reply = line(fx.post("/api/members/admin", &admin_body(true)).await);
        assert_eq!(reply, "409 not_member");
    }

    #[tokio::test]
    async fn a_body_without_id_is_400_and_the_node_is_not_called() {
        let fx = setup(Answer::List(managed_reply(&[])));
        let reply = line(fx.post("/api/members/admin", r#"{"admin":true}"#).await);
        assert_eq!(reply, "400 bad_request");
        assert!(fx.seen().is_empty(), "node was called: {:?}", fx.seen());
    }

    #[tokio::test]
    async fn a_body_without_the_admin_flag_is_400_and_the_node_is_not_called() {
        // assumed: a missing or non-boolean `admin` is a bad request, like a missing `id`
        let fx = setup(Answer::List(managed_reply(&[])));
        for text in [
            json!({"id": target()}),
            json!({"id": target(), "admin": "yes"}),
        ] {
            let reply = line(fx.post("/api/members/admin", &text.to_string()).await);
            assert_eq!(reply, "400 bad_request", "body {text}");
        }
        assert!(fx.seen().is_empty(), "node was called: {:?}", fx.seen());
    }
}

mod delete_network {
    use super::*;

    #[tokio::test]
    async fn without_confirmation_is_422_and_the_node_is_not_called() {
        let fx = setup(Answer::List(managed_reply(&[])));
        for text in ["{}", r#"{"confirm":false}"#, r#"{"confirm":"yes"}"#] {
            let reply = line(fx.post("/api/network/delete", text).await);
            assert_eq!(reply, "422 invalid", "body {text}");
        }
        assert!(fx.seen().is_empty(), "node was called: {:?}", fx.seen());
    }

    #[tokio::test]
    async fn with_confirmation_the_node_closes_the_network() {
        // given
        let fx = setup(Answer::List(managed_reply(&[])));
        // when
        let reply = line(fx.post("/api/network/delete", r#"{"confirm":true}"#).await);
        // then
        assert_eq!(reply, "200 -");
        assert_eq!(fx.seen(), vec!["close".to_string()]);
    }

    #[tokio::test]
    async fn a_member_without_the_role_is_refused_as_not_admin() {
        let fx = setup(not_admin());
        let reply = line(fx.post("/api/network/delete", r#"{"confirm":true}"#).await);
        assert_eq!(reply, "409 not_admin");
    }
}

mod list_fields {
    use super::*;

    #[tokio::test]
    async fn the_list_names_the_administrators_and_my_role() {
        // given: I am an administrator, and so is the second row
        let fx = setup(Answer::List(managed_reply(&[full_id("a1b2c3d4")])));
        // when
        let (status, body) = fx.get("/api/members").await;
        // then
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["admins"], json!([full_id("a1b2c3d4")]));
        assert_eq!(body["me_admin"], true);
    }

    #[tokio::test]
    async fn a_row_tells_whether_the_member_is_an_administrator() {
        // given: only the first row is an administrator
        let mut reply = managed_reply(&[full_id("a1b2c3d4")]);
        reply.members[0].admin = true;
        let fx = setup(Answer::List(reply));
        // when
        let (_, body) = fx.get("/api/members").await;
        // then
        let admin_flags: Vec<&Value> = body["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| &row["admin"])
            .collect();
        assert_eq!(admin_flags, [&json!(true), &json!(false), &json!(false)]);
    }
}

// --- helpers ---

/// A list where I am an administrator; `admins` are the full ids of the administrators.
fn managed_reply(admins: &[String]) -> MembersReply {
    MembersReply {
        admins: admins.to_vec(),
        me_admin: true,
        root: None,
        ..sample_reply()
    }
}

fn admin_body(admin: bool) -> String {
    json!({"id": target(), "admin": admin}).to_string()
}

/// A 64-hex id padded from a short prefix, as the rows of `sample_reply` carry it.
fn full_id(prefix: &str) -> String {
    format!("{prefix}{}", "0".repeat(64 - prefix.len()))
}
