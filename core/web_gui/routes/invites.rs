//! Open invites of a network: the list and the edit of an invite's "for whom" note (admins only; the node decides).
//! Routes: `GET {base}/network/invites`, `POST {base}/network/invites/label` with `{"hash", "for"}`.

use std::sync::Arc;

use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde_json::{Map, Value};

use super::helpers::{action_error, bad_method, body_obj};
use super::peers::check_label;
use super::slot_param::SlotParam;
use crate::web_gui::http::json_err;
use crate::web_gui::{App, InvitesReply, MemberError};

/// Length of the invite hash in hex characters.
const HASH_HEX_CHARS: usize = 64;

pub(super) fn register_invites(router: Router<Arc<App>>, base: &str) -> Router<Arc<App>> {
    router
        .route(
            &format!("{base}/network/invites"),
            get(list_handler).fallback(bad_method),
        )
        .route(
            &format!("{base}/network/invites/label"),
            post(label_handler).fallback(bad_method),
        )
}

/// The reply of the list or of the edit; a refusal is 409 with its code.
fn reply(result: Result<InvitesReply, MemberError>) -> Response {
    match result {
        Ok(body) => (StatusCode::OK, axum::Json(body)).into_response(),
        Err(MemberError::Action(e)) => action_error(e),
        Err(MemberError::Refused { code, message }) => {
            json_err(StatusCode::CONFLICT, code, &message)
        }
    }
}

async fn list_handler(SlotParam(slot): SlotParam) -> Response {
    reply(slot.node.open_invites().await)
}

async fn label_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    let Ok(map) = body_obj(&bytes) else {
        return bad_request("body must be a JSON object");
    };
    let hash = match hash_of(&map) {
        Ok(h) => h,
        Err(resp) => return resp,
    };
    match label_of(&map) {
        Ok(label) => reply(slot.node.relabel_invite(hash, label).await),
        Err(resp) => resp,
    }
}

/// The `hash` field: 64 hex characters, lowercased.
fn hash_of(map: &Map<String, Value>) -> Result<String, Response> {
    match map.get("hash") {
        Some(Value::String(h)) if is_hash(h.trim()) => Ok(h.trim().to_ascii_lowercase()),
        _ => Err(bad_request("field hash must be 64 hex characters")),
    }
}

fn is_hash(text: &str) -> bool {
    text.len() == HASH_HEX_CHARS && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The `for` field: absent, `null` or blank clears the note; a non-string is a `bad_request`.
fn label_of(map: &Map<String, Value>) -> Result<Option<String>, Response> {
    match map.get("for") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => check_label(text).map_err(action_error),
        Some(_) => Err(bad_request("field for must be a string")),
    }
}

fn bad_request(message: &str) -> Response {
    json_err(StatusCode::BAD_REQUEST, "bad_request", message)
}

#[cfg(test)]
#[path = "invites_tests.rs"]
mod tests;
