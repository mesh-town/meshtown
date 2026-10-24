//! Response → `Reply` per the contract (`docs/WEB-API.md`): 200 with a JSON object body → `Ok`; any other 200 →
//! `OtherStatus(200)`; 202 → `Accepted`; 401 → `Unauthorized`; 503 with `error == "stopping"` in the body →
//! `Stopping`; 409 with `error == "not_admin"` → `NotAdmin`; anything else, including 403 `bad_host`/`bad_origin`, → `OtherStatus(code)`.

use serde_json::Value;

use super::types::Reply;
use crate::gui::tray::app::snapshot::lenient;

pub(crate) fn reply_from_response(status: u16, body: &[u8]) -> Reply {
    match status {
        200 => match serde_json::from_slice::<Value>(body) {
            Ok(value) if value.is_object() => Reply::Ok(value),
            _ => Reply::OtherStatus(200),
        },
        202 => Reply::Accepted,
        401 => Reply::Unauthorized,
        503 => stopping_or_other(body),
        409 => conflict_or_other(body),
        other => Reply::OtherStatus(other),
    }
}

/// Error response body: `{"error": "…"}`.
#[derive(Default, serde::Deserialize)]
#[serde(default)]
struct ErrorBody {
    #[serde(deserialize_with = "lenient::opt_string")]
    error: Option<String>,
}

/// A 409 with `error == "not_admin"` means the network lets only admins do this; any other 409 stays `OtherStatus(409)`.
fn conflict_or_other(body: &[u8]) -> Reply {
    let value = serde_json::from_slice::<Value>(body).unwrap_or(Value::Null);
    let error: ErrorBody = lenient::from_object(&value);
    match error.error.as_deref() {
        Some("not_admin") => Reply::NotAdmin,
        _ => Reply::OtherStatus(409),
    }
}

/// A 503 with `error == "stopping"` means the node is stopping; any other body, or non-JSON, gives `OtherStatus(503)`.
fn stopping_or_other(body: &[u8]) -> Reply {
    let value = serde_json::from_slice::<Value>(body).unwrap_or(Value::Null);
    let error: ErrorBody = lenient::from_object(&value);
    match error.error.as_deref() {
        Some("stopping") => Reply::Stopping,
        _ => Reply::OtherStatus(503),
    }
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
