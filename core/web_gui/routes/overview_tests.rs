//! `/api/overview`: the list of networks with the state and the members of every slot.

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::NetSlotId;
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};

fn host(ids: &[&str]) -> std::sync::Arc<FakeHost> {
    let slots = ids
        .iter()
        .map(|id| slot_handle(id, SnapNode::new(Default::default()), config_with_key(None)))
        .collect();
    FakeHost::new(slots)
}

#[tokio::test]
async fn one_slot_matches_the_separate_replies() {
    // given
    let router = test_app(host(&["main"])).0;

    // when
    let (status, overview) = send(&router, Method::GET, "/api/overview", "").await;
    let (_, nets) = send(&router, Method::GET, "/api/nets", "").await;
    let (_, state) = send(&router, Method::GET, "/api/state", "").await;

    // then
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview["api"], 2);
    assert_eq!(overview["nets"], nets["nets"]);
    let mut expected = state.clone();
    expected.as_object_mut().unwrap().remove("nets");
    expected.as_object_mut().unwrap().remove("notices");
    let mut got = overview["slots"][0]["state"].clone();
    for value in [&mut expected, &mut got] {
        value.as_object_mut().unwrap().remove("now");
        value["node"].as_object_mut().unwrap().remove("via_iface");
    }
    assert_eq!(got, expected);
    assert_eq!(overview["slots"][0]["slot"], "main");
    assert_eq!(overview["slots"][0]["members"], Value::Null);
    assert_eq!(overview["restart_slots"], json!([]));
}

#[tokio::test]
async fn slots_follow_the_order_of_nets() {
    // given
    let router = test_app(host(&["main", "zz2222", "ab2c3d"])).0;

    // when
    let (_, body) = send(&router, Method::GET, "/api/overview", "").await;

    // then
    let slots: Vec<&str> = body["slots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["slot"].as_str().unwrap())
        .collect();
    assert_eq!(slots, ["main", "zz2222", "ab2c3d"]);
    assert_eq!(body["nets"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn a_broken_slot_comes_last_without_state_or_members() {
    // given
    let fake = host(&["main"]);
    fake.set_broken(vec![NetSlotId::parse("ab2c3d").unwrap()]);
    let router = test_app(fake).0;

    // when
    let (_, body) = send(&router, Method::GET, "/api/overview", "").await;

    // then
    let last = body["slots"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(
        last,
        json!({"slot": "ab2c3d", "state": null, "members": null})
    );
    assert_eq!(body["restart_slots"], json!([]));
    assert_eq!(body["nets"][1]["status"], "broken");
}

#[tokio::test]
async fn the_overview_is_not_a_network_route() {
    // given
    let router = test_app(host(&["main", "ab2c3d"])).0;

    // when
    let (status, body) = send(&router, Method::GET, "/api/net/ab2c3d/overview", "").await;

    // then
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}
