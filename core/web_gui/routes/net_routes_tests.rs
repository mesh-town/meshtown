//! Slot addressing of the network routes: the prefix picks the slot, `/api/…` is the primary one.

use std::sync::Arc;

use axum::http::{Method, StatusCode};

use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};

struct Fixture {
    router: axum::Router,
    main: Arc<SnapNode>,
    other: Arc<SnapNode>,
}

fn fixture() -> Fixture {
    let main = SnapNode::new(Default::default());
    let other = SnapNode::new(Default::default());
    let slots = vec![
        slot_handle("main", main.clone(), config_with_key(None)),
        slot_handle("ab2c3d", other.clone(), config_with_key(None)),
    ];
    let router = test_app(FakeHost::new(slots)).0;
    Fixture {
        router,
        main,
        other,
    }
}

#[tokio::test]
async fn prefixed_restart_reaches_only_that_slot() {
    // given
    let f = fixture();
    // when
    let (status, _) = send(
        &f.router,
        Method::POST,
        "/api/net/ab2c3d/actions/restart",
        "",
    )
    .await;
    // then
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(f.other.restarts(), 1, "restarts of ab2c3d");
    assert_eq!(f.main.restarts(), 0, "restarts of the primary slot");
}

#[tokio::test]
async fn unknown_slot_is_404_unknown_net() {
    // given
    let f = fixture();
    // when
    let (status, body) = send(
        &f.router,
        Method::POST,
        "/api/net/zzzzzz/actions/restart",
        "",
    )
    .await;
    let (bad, bad_body) = send(
        &f.router,
        Method::POST,
        "/api/net/not-an-id/actions/restart",
        "",
    )
    .await;
    // then
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "unknown_net");
    assert_eq!(bad, StatusCode::NOT_FOUND);
    assert_eq!(bad_body["error"], "unknown_net");
    assert_eq!(f.main.restarts() + f.other.restarts(), 0, "restarts");
}

#[tokio::test]
async fn both_addresses_of_the_primary_slot_act_the_same() {
    // given
    let f = fixture();
    // when
    let (plain, _) = send(&f.router, Method::POST, "/api/actions/restart", "").await;
    let (named, _) = send(&f.router, Method::POST, "/api/net/main/actions/restart", "").await;
    // then
    assert_eq!((plain, named), (StatusCode::ACCEPTED, StatusCode::ACCEPTED));
    assert_eq!(f.main.restarts(), 2, "restarts of the primary slot");
    assert_eq!(f.other.restarts(), 0, "restarts of ab2c3d");
}
