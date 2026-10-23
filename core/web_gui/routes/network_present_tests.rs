//! Joining or creating a network on a slot: a slot with another network refuses, a network that another
//! slot works with is not taken twice, the slot's own network is accepted again.

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::NetSlotId;
use crate::members::network_key::format_key_string;
use crate::members::words::generate_tail;
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost};
use crate::web_gui::{HostControl, SlotHandle};

fn new_key(label: &str) -> String {
    format_key_string(Some(label), &generate_tail().unwrap())
}

struct Fixture {
    router: axum::Router,
    host: Arc<FakeHost>,
    key_a: String,
    key_b: String,
}

/// The primary slot works with network A; `ab2c3d` has none.
fn fixture() -> Fixture {
    let (key_a, key_b) = (new_key("alpha"), new_key("beta"));
    let main = slot_handle(
        "main",
        crate::web_gui::test_host::SnapNode::new(Default::default()),
        config_with_key(Some(&key_a)),
    );
    let empty = slot_handle(
        "ab2c3d",
        crate::web_gui::test_host::SnapNode::new(Default::default()),
        config_with_key(None),
    );
    let host = FakeHost::new(vec![main, empty]);
    let router = test_app(host.clone()).0;
    Fixture {
        router,
        host,
        key_a,
        key_b,
    }
}

fn slot_of(f: &Fixture, id: &str) -> SlotHandle {
    f.host.slot(&NetSlotId::parse(id).unwrap()).unwrap()
}

fn key_of(f: &Fixture, id: &str) -> Option<String> {
    let slot = slot_of(f, id);
    let cfg = slot.config.lock().unwrap();
    cfg.current_network_key.clone()
}

async fn join(f: &Fixture, path: &str, key: &str) -> (StatusCode, Value) {
    let body = json!({"key": key}).to_string();
    send(&f.router, Method::POST, path, &body).await
}

#[tokio::test]
async fn a_network_of_another_slot_is_not_taken_twice() {
    // given
    let f = fixture();
    // when
    let (status, body) = join(&f, "/api/net/ab2c3d/network/join", &f.key_a).await;
    // then
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"], "net_already_added");
    assert_eq!(body["slot"], "main");
    assert_eq!(key_of(&f, "ab2c3d"), None);
}

#[tokio::test]
async fn a_slot_with_another_network_refuses_join_and_create() {
    // given
    let f = fixture();
    for (path, body) in [
        ("/api/network/join", json!({"key": f.key_b}).to_string()),
        ("/api/network/create", json!({"label": "gamma"}).to_string()),
    ] {
        // when
        let (status, reply) = send(&f.router, Method::POST, path, &body).await;
        // then
        assert_eq!(status, StatusCode::CONFLICT, "{path}: {reply}");
        assert_eq!(reply["error"], "network_present", "{path}");
    }
    assert_eq!(key_of(&f, "main"), Some(f.key_a.clone()));
}

#[tokio::test]
async fn the_own_network_is_accepted_again() {
    // given
    let f = fixture();
    // when
    let (status, body) = join(&f, "/api/network/join", &f.key_a).await;
    // then
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
}

#[tokio::test]
async fn an_empty_slot_takes_a_new_network_with_or_without_keep_ports() {
    // given
    let f = fixture();
    // when
    let body = json!({"key": f.key_b, "keep_ports": true}).to_string();
    let (status, reply) = send(
        &f.router,
        Method::POST,
        "/api/net/ab2c3d/network/join",
        &body,
    )
    .await;
    // then
    assert_eq!(status, StatusCode::ACCEPTED, "{reply}");
    assert_eq!(key_of(&f, "ab2c3d"), Some(f.key_b.clone()));
}

#[tokio::test]
async fn an_empty_slot_creates_a_network() {
    // given
    let f = fixture();
    // when
    let body = json!({"label": "gamma"}).to_string();
    let (status, reply) = send(
        &f.router,
        Method::POST,
        "/api/net/ab2c3d/network/create",
        &body,
    )
    .await;
    // then
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert_eq!(
        key_of(&f, "ab2c3d"),
        Some(reply["key"].as_str().unwrap().to_string())
    );
    assert_eq!(key_of(&f, "main"), Some(f.key_a.clone()));
}
