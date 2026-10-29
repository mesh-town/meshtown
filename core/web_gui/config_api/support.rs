//! Shared constants and small `config_api` helpers: unavailability reasons, descent along a
//! path in JSON, wrapping a response in `Response`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

pub(crate) const AUTOSTART_REASON: &str = "only in the desktop app with the tray icon";
pub(crate) const FULL_TUNNEL_REASON: &str = "coming in a later version";

/// Descent along path parts via `Value::get`.
pub(super) fn get_path<'v>(doc: &'v Value, path: &str) -> Option<&'v Value> {
    let mut cur = doc;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

pub(super) fn field_entry(path: &str, message: &str) -> Value {
    serde_json::json!({"path": path, "message": message})
}

pub(super) fn json_response(status: StatusCode, body: Value) -> Response {
    (status, axum::Json(body)).into_response()
}
