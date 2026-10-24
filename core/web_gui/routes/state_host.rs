//! Keys of `/api/state` that name the slot and list the networks of the process.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use super::helpers::body_obj;
use crate::web_gui::http::json_err;

use crate::config::config_file::NetSlotId;
use crate::web_gui::net_brief::NetBrief;
use crate::web_gui::{App, Notice};

/// The api version of the multi-network replies.
pub(super) const API: u32 = 2;

/// Marks the state with the api version and the slot it describes.
pub(super) fn apply_slot(value: &mut Value, slot: &NetSlotId) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.insert("api".into(), json!(API));
    obj.insert("slot".into(), json!(slot.to_string()));
}

/// Adds the list of networks of the process to the state of `slot`.
pub(super) fn apply_host(value: &mut Value, slot: &NetSlotId, nets: Vec<NetBrief>) {
    apply_slot(value, slot);
    if let Some(obj) = value.as_object_mut() {
        obj.insert("nets".into(), json!(nets));
    }
}

/// Adds the host's notices (oldest first; `[]` when there are none) to a state or an overview.
pub(super) fn apply_notices(value: &mut Value, notices: Vec<Notice>) {
    if let Some(obj) = value.as_object_mut() {
        obj.insert("notices".into(), json!(notices));
    }
}

/// `POST /api/notices/ack {id}`: 200 `{}`; no such notice 404 `not_found`; no `id` number 422 `invalid`.
pub(super) async fn notices_ack_handler(State(app): State<Arc<App>>, body: Bytes) -> Response {
    let map = match body_obj(&body) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let Some(id) = map.get("id").and_then(Value::as_u64) else {
        return invalid_id();
    };
    match app.host.ack_notice(id) {
        true => (StatusCode::OK, axum::Json(json!({}))).into_response(),
        false => json_err(StatusCode::NOT_FOUND, "not_found", "no such notice"),
    }
}

fn invalid_id() -> Response {
    let body = json!({
        "error": "invalid",
        "message": "id must be a notice number",
        "fields": [{"path": "id", "message": "must be a notice number"}],
    });
    (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
}

#[cfg(test)]
#[path = "notices_tests.rs"]
mod notices_tests;
