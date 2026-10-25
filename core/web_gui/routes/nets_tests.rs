//! `/api/nets`: the list and the way to add a network.

use axum::http::{Method, StatusCode};
use serde_json::json;

use crate::config::config_file::NetSlotId;
use crate::web_gui::test_host::{
    config_with_key, send, slot_handle, test_app, FakeHost, HostCall, SnapNode,
};
use crate::web_gui::{AddedSlot, HostError};

fn host() -> std::sync::Arc<FakeHost> {
    let main = slot_handle(
        "main",
        SnapNode::new(Default::default()),
        config_with_key(None),
    );
    FakeHost::new(vec![main])
}

fn added(fresh: bool) -> Result<AddedSlot, HostError> {
    Ok(AddedSlot {
        id: NetSlotId::parse("main").unwrap(),
        fresh,
    })
}

#[tokio::test]
async fn the_list_has_the_api_and_the_briefs() {
    // given
    let router = test_app(host()).0;

    // when
    let (status, body) = send(&router, Method::GET, "/api/nets", "").await;

    // then
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["api"], 2);
    assert_eq!(body["nets"][0]["id"], "main");
}

#[tokio::test]
async fn a_new_slot_is_201_and_a_handed_back_one_is_200() {
    // given
    let fake = host();
    let router = test_app(fake.clone()).0;

    // when
    fake.set_add(added(true));
    let (created, body) = send(&router, Method::POST, "/api/nets", "").await;
    fake.set_add(added(false));
    let (again, again_body) = send(&router, Method::POST, "/api/nets", "{}").await;

    // then
    assert_eq!(created, StatusCode::CREATED);
    assert_eq!(body["slot"], "main");
    assert_eq!(body["nets"][0]["id"], "main");
    assert_eq!(body["fresh"], true);
    assert_eq!(again, StatusCode::OK);
    assert_eq!(again_body["fresh"], false);
    assert_eq!(fake.calls(), vec![HostCall::Add, HostCall::Add]);
}

#[tokio::test]
async fn a_body_that_is_not_json_is_refused_before_the_host() {
    // given
    let fake = host();
    let router = test_app(fake.clone()).0;

    // when
    let (status, body) = send(&router, Method::POST, "/api/nets", "not json").await;

    // then
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_json");
    assert!(fake.calls().is_empty());
}

#[tokio::test]
async fn host_errors_have_their_codes() {
    // given
    let fake = host();
    let router = test_app(fake.clone()).0;
    let cases = [
        (
            HostError::TooManyNets,
            StatusCode::CONFLICT,
            "too_many_nets",
        ),
        (HostError::UnknownNet, StatusCode::NOT_FOUND, "unknown_net"),
        (
            HostError::AlreadyAdded,
            StatusCode::CONFLICT,
            "net_already_added",
        ),
        (
            HostError::Busy("stopping".into()),
            StatusCode::CONFLICT,
            "busy",
        ),
        (
            HostError::Io("disk".into()),
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
        ),
    ];

    for (err, code, name) in cases {
        // when
        fake.set_add(Err(err));
        let (status, body) = send(&router, Method::POST, "/api/nets", "").await;

        // then
        assert_eq!((status, body["error"].as_str()), (code, Some(name)));
        let exact = match name {
            "too_many_nets" => Some("at most 8 networks on this computer"),
            "unknown_net" => Some("no such network on this computer"),
            "net_already_added" => Some("this network is already on this computer"),
            _ => None,
        };
        if let Some(text) = exact {
            assert_eq!(body["message"].as_str(), Some(text), "{name}");
        }
    }
}

#[tokio::test]
async fn the_limit_is_in_the_reply() {
    // given
    let router = test_app(host()).0;

    // when
    let (_, body) = send(&router, Method::POST, "/api/nets", "").await;

    // then
    assert_eq!(body["limit"], json!(8));
}

#[tokio::test]
async fn delete_has_no_route() {
    // given
    let router = test_app(host()).0;

    // when
    let (status, _) = send(&router, Method::DELETE, "/api/nets", "").await;

    // then
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}
