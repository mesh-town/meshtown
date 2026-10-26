//! Tests of the member-list web API for requests to join: the `admit` route and the JSON of a
//! waiting node. Driven through the real router with a fake node (`fixture`).

use axum::http::StatusCode;
use serde_json::json;

use super::fixture::*;
use crate::web_gui::members::dto::{AdmissionInfo, MembersReply};
use crate::web_gui::ActionError;

const ACTIONS: [&str; 1] = ["admit"];

#[tokio::test]
async fn admit_route_maps_codes() {
    // given / when / then: a bad body is 400, a refusal is 409 with its code, a bad id is 422
    let fx = setup(Answer::List(sample_reply()));
    assert_eq!(
        line(fx.post("/api/members/admit", "{}").await),
        "400 bad_request"
    );
    assert_eq!(
        line(fx.post("/api/members/admit", "[]").await),
        "400 bad_request"
    );
    assert!(fx.seen().is_empty(), "the node was called: {:?}", fx.seen());
    let ok = fx.post("/api/members/admit", &id_body()).await;
    assert_eq!(ok.0, StatusCode::OK);
    assert_eq!(fx.seen(), vec![format!("admit:{}", target())]);
    let refused = setup(Answer::Refused {
        code: "not_pending",
        message: "This member isn't waiting for confirmation",
    });
    assert_eq!(
        line(refused.post("/api/members/admit", &id_body()).await),
        "409 not_pending"
    );
    let invalid = setup(Answer::Action(ActionError::Invalid("id".to_string())));
    assert_eq!(
        line(invalid.post("/api/members/admit", &id_body()).await),
        "422 invalid"
    );
    let busy = setup(Answer::Action(ActionError::Busy("starting".to_string())));
    assert_eq!(
        line(busy.post("/api/members/admit", &id_body()).await),
        "409 busy"
    );
}

#[tokio::test]
async fn admit_routes_accept_only_post() {
    // given / when / then: a GET is a wrong method, not a 404
    let fx = setup(Answer::List(sample_reply()));
    for action in ACTIONS {
        let (status, _) = fx.get(&format!("/api/members/{action}")).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{action}");
    }
}

#[test]
fn members_json_has_admission_fields() {
    // given: a reply with one waiting node and my own request
    let waiting = AdmissionInfo {
        label: Some("Anna's laptop".into()),
    };
    let mut reply = sample_reply();
    reply.me_pending = true;
    reply.me_admission = Some(AdmissionInfo { label: None });
    reply.counts.pending = 1;
    reply.members[1].status = "pending";
    reply.members[1].admission = Some(waiting);
    // when
    let json = serde_json::to_value(&reply).unwrap();
    // then: the keys of the contract are all there
    assert_eq!(json["me_pending"], json!(true));
    assert_eq!(json["me_admission"], json!({"label": null}));
    assert_eq!(json["counts"]["pending"], json!(1));
    let row = &json["members"][1];
    assert_eq!(row["status"], json!("pending"));
    assert_eq!(row["admission"]["label"], json!("Anna's laptop"));
    assert_eq!(row["admission"].as_object().unwrap().len(), 1);
}

#[test]
fn a_plain_reply_has_null_admission_fields() {
    // given / when: a reply without requests
    let json = serde_json::to_value(sample_reply() as MembersReply).unwrap();
    // then: the keys are present with defaults, so the page need not guess
    assert_eq!(json["me_pending"], json!(false));
    assert_eq!(json["me_admission"], json!(null));
    assert_eq!(json["members"][0]["admission"], json!(null));
}
