//! Node addresses and the network key/invite: `/api/peer-addresses`, `/api/network/key`,
//! `/api/network/invite`.

use axum::body::Bytes;
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use crate::invite::store::FOR_WHOM_MAX_CHARS;
use crate::web_gui::http::{json_err, query_param};
use crate::web_gui::invite_qr;
use crate::web_gui::ActionError;

use super::helpers::{action_error, body_obj};
use super::slot_param::SlotParam;

pub(super) async fn peer_addresses_get_handler(SlotParam(slot): SlotParam) -> Response {
    let snap = slot.node.snapshot();
    let body = json!({"lines": snap.transport.self_addresses});
    (StatusCode::OK, axum::Json(body)).into_response()
}

pub(super) async fn peer_addresses_post_handler(
    SlotParam(slot): SlotParam,
    bytes: Bytes,
) -> Response {
    let map = match body_obj(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let text = match map.get("text") {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(_) => {
            return json_err(
                StatusCode::BAD_REQUEST,
                "bad_request",
                "field text is required",
            )
        }
    };
    let mut added: u64 = 0;
    let mut rejected = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        match slot.node.add_peer_address(line) {
            Ok(()) => added += 1,
            Err(message) => rejected.push(json!({"line": line, "message": message})),
        }
    }
    let body = json!({"added": added, "rejected": rejected});
    (StatusCode::OK, axum::Json(body)).into_response()
}

pub(super) async fn network_key_handler(SlotParam(slot): SlotParam) -> Response {
    let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    let body = json!({"key": cfg.current_network_key});
    (StatusCode::OK, axum::Json(body)).into_response()
}

/// The optional `for` note of an invite: trimmed; blank or absent is none; too long or with a control character
/// is invalid.
fn invite_label(uri: &Uri) -> Result<Option<String>, ActionError> {
    match query_param(uri, "for") {
        Some(raw) => check_label(&raw),
        None => Ok(None),
    }
}

/// Control characters and the Unicode line and paragraph separators, which `char::is_control` lets through.
fn is_line_break_or_control(c: char) -> bool {
    c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')
}

/// A "for whom" note checked: trimmed; blank is none; too long or with a control character is invalid.
pub(super) fn check_label(text: &str) -> Result<Option<String>, ActionError> {
    let text = text.trim();
    if text.chars().count() > FOR_WHOM_MAX_CHARS {
        return Err(ActionError::Invalid(format!(
            "for: at most {FOR_WHOM_MAX_CHARS} characters"
        )));
    }
    if text.chars().any(is_line_break_or_control) {
        return Err(ActionError::Invalid(
            "for: must not contain control characters".to_string(),
        ));
    }
    Ok(Some(text.to_string()).filter(|s| !s.is_empty()))
}

pub(super) async fn invite_name_handler() -> Response {
    match crate::members::words::random_node_name() {
        Ok(name) => axum::Json(json!({"name": name})).into_response(),
        Err(_) => json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "name_unavailable",
            "Could not suggest an invite name",
        ),
    }
}

pub(super) async fn network_invite_handler(SlotParam(slot): SlotParam, uri: Uri) -> Response {
    let label = match invite_label(&uri) {
        Ok(label) => label,
        Err(e) => return action_error(e),
    };
    let pending = match slot.node.invite_pending(label.as_deref()) {
        Ok(p) => p,
        Err(e) => return action_error(e),
    };
    match pending.finish_dealt().await {
        Ok(dealt) => {
            let body = invite_qr::with_qr(dealt.api_body());
            (StatusCode::OK, axum::Json(body)).into_response()
        }
        Err(e) => action_error(e),
    }
}

#[cfg(test)]
#[path = "peers_invite_tests.rs"]
mod invite_tests;

#[cfg(test)]
#[path = "peers_label_tests.rs"]
mod label_tests;
