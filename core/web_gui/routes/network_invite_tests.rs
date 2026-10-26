//! `POST /api/network/join` with an invite string: an invite that cannot be used is refused before any
//! network is touched.

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::web_gui::routes::invite_fixture::{invite_into, stale_invite_into, word_key};
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};
use crate::web_gui::SlotHandle;

fn empty_slot() -> SlotHandle {
    slot_handle(
        "main",
        SnapNode::new(Default::default()),
        config_with_key(None),
    )
}

async fn when_join(slot: &SlotHandle, text: &str) -> (StatusCode, Value) {
    let router = test_app(FakeHost::new(vec![slot.clone()])).0;
    let body = json!({"key": text}).to_string();
    send(&router, Method::POST, "/api/network/join", &body).await
}

fn key_of(slot: &SlotHandle) -> Option<String> {
    slot.config.lock().unwrap().current_network_key.clone()
}

#[tokio::test]
async fn an_expired_invite_is_refused_and_writes_no_key() {
    let slot = empty_slot();
    let invite = stale_invite_into(&word_key("beta"));
    let (status, reply) = when_join(&slot, &invite).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
    assert_eq!(reply["error"], "invalid_key", "{reply}");
    assert_eq!(key_of(&slot), None);
}

#[tokio::test]
async fn a_cut_invite_is_refused_and_writes_no_key() {
    let slot = empty_slot();
    let invite = invite_into(&word_key("beta"));
    let (status, reply) = when_join(&slot, &invite[..invite.len() - 6]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
    assert_eq!(reply["error"], "invalid_key", "{reply}");
    assert_eq!(key_of(&slot), None);
}
