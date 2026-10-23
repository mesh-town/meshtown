//! Shared handler helpers: request body parsing, standard responses, the node-stop guard.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Map, Value};

use crate::web_gui::http::json_err;
use crate::web_gui::{ActionError, App};

/// The request body as a JSON object: an empty body is an empty map; non-JSON is 400 `bad_json`; JSON
/// that is not an object is also an empty map (fields are looked up via `map_string_field`/`map.get`, and their absence is
/// handled there).
pub(crate) fn body_obj(bytes: &[u8]) -> Result<Map<String, Value>, Response> {
    if bytes.is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Ok(Map::new()),
        Err(_) => Err(json_err(
            StatusCode::BAD_REQUEST,
            "bad_json",
            "body is not JSON",
        )),
    }
}

/// A string field of the body by name: missing or not a string — 400 `bad_request` with a message by the field
/// name.
pub(super) fn map_string_field(map: &Map<String, Value>, name: &str) -> Result<String, Response> {
    let message = match name {
        "domain" => "field domain is required",
        "text" => "field text is required",
        "key" => "field key is required",
        "label" => "field label is required",
        other => {
            return Err(json_err(
                StatusCode::BAD_REQUEST,
                "bad_request",
                &format!("field {other} is required"),
            ))
        }
    };
    match map.get(name) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        _ => Err(json_err(StatusCode::BAD_REQUEST, "bad_request", message)),
    }
}

/// 422 `invalid_key`: `word` only when `err` wraps `BadWord` (`err.downcast_ref`),
/// otherwise there is no field at all (not `null`); `message` comes from `BadWord`, otherwise `fallback_message`.
pub(super) fn invalid_key_response(err: &anyhow::Error, fallback_message: &str) -> Response {
    let body = match err.downcast_ref::<crate::members::network_key::BadWord>() {
        Some(bad) => json!({"error": "invalid_key", "message": bad.message, "word": bad.word}),
        None => json!({"error": "invalid_key", "message": fallback_message}),
    };
    (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
}

/// Converts a node's `ActionError` into a page response.
pub(crate) fn action_error(err: ActionError) -> Response {
    match err {
        ActionError::Busy(message) | ActionError::NoNetwork(message) => {
            json_err(StatusCode::CONFLICT, "busy", &message)
        }
        ActionError::NotFound(message) => json_err(StatusCode::NOT_FOUND, "not_found", &message),
        ActionError::Invalid(message) => {
            let body = json!({
                "error": "invalid",
                "message": message,
                "fields": [],
            });
            (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
        }
        ActionError::Internal(message) => {
            json_err(StatusCode::INTERNAL_SERVER_ERROR, "internal", &message)
        }
    }
}

/// Top-level `Router::fallback` — no such address.
pub(super) async fn fallback_404() -> Response {
    json_err(StatusCode::NOT_FOUND, "not_found", "no such address")
}

/// `.fallback(...)` on every route — the address exists, the method is not supported.
pub(crate) async fn bad_method() -> Response {
    json_err(
        StatusCode::METHOD_NOT_ALLOWED,
        "bad_method",
        "method not supported at this address",
    )
}

/// A separate middleware layer: while the node is stopping, mutating requests to `/api/` are rejected with
/// 503 `stopping`; static files, `GET` and everything outside `/api/` keep going.
pub(super) async fn reject_mutations_while_stopping(
    State(app): State<Arc<App>>,
    req: Request,
    next: Next,
) -> Response {
    let mutating = matches!(
        *req.method(),
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    );
    if mutating && req.uri().path().starts_with("/api/") && app.stopping.load(Ordering::SeqCst) {
        return json_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "stopping",
            "node is stopping",
        );
    }
    next.run(req).await
}
