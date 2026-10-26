//! `POST /api/network/join` with text that is not a usable key or invite: keys and invites of older programs
//! are refused with their own text, garbage with the generic one.

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::invite::{INVITE_PREFIX, TEXT_V3_INVITE};
use crate::members::network_key::LEGACY_KEY_TEXT;
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};

/// A key in the raw form of an older program.
const OLD_MACHINE_KEY: &str = "work--mesh1-3lzabdyowamq5nn6glcv5gwzecclktknbnl3lutynrtzgpco73ta";
/// A key of an older program in the words form: label, two words and a group, no creator.
const OLD_WORD_KEY: &str = "work--abacus-abdomen-0123456789abcdefghjkm";

async fn when_join(text: &str) -> (StatusCode, Value) {
    let slot = slot_handle(
        "main",
        SnapNode::new(Default::default()),
        config_with_key(None),
    );
    let router = test_app(FakeHost::new(vec![slot])).0;
    let body = json!({"key": text}).to_string();
    send(&router, Method::POST, "/api/network/join", &body).await
}

fn version_3_invite() -> String {
    format!(
        "{INVITE_PREFIX}{}",
        data_encoding::BASE32_NOPAD.encode(&[3, 1, 2, 3, 4])
    )
}

#[tokio::test]
async fn an_old_machine_key_is_refused_as_outdated() {
    let (status, reply) = when_join(OLD_MACHINE_KEY).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
    assert_eq!(reply["error"], "invalid_key", "{reply}");
    assert_eq!(reply["message"], LEGACY_KEY_TEXT, "{reply}");
}

#[tokio::test]
async fn an_old_word_key_is_refused_as_outdated() {
    let (status, reply) = when_join(OLD_WORD_KEY).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
    assert_eq!(reply["error"], "invalid_key", "{reply}");
    assert_eq!(reply["message"], LEGACY_KEY_TEXT, "{reply}");
}

#[tokio::test]
async fn a_version_3_invite_is_refused_as_outdated() {
    let (status, reply) = when_join(&version_3_invite()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
    assert_eq!(reply["error"], "invalid_key", "{reply}");
    assert_eq!(reply["message"], TEXT_V3_INVITE, "{reply}");
}

#[tokio::test]
async fn garbage_is_not_called_outdated() {
    let (status, reply) = when_join("hello").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
    assert_eq!(reply["error"], "invalid_key", "{reply}");
    assert_ne!(reply["message"], LEGACY_KEY_TEXT, "{reply}");
}
