//! The JSON API helper of the web listener scenarios: `api`.
//!
//! What it does: sends a request with the token header and parses the JSON answer.
//! What it does not do: it does not expose headers (see `web_listener_headers.rs`).
//! Links: included as a sibling of `common` by the scenarios that call the API; takes the base through `super::common`.

use std::time::Duration;

use serde_json::Value;

use super::common::http_within;

/// A wrapper over `http` for the JSON API: the token in `X-Meshtown-Token`, the body and the response are JSON (an empty response body
/// → `Value::Null`).
pub(crate) fn api(
    port: u16,
    token: &str,
    method: &str,
    path: &str,
    json: Option<Value>,
) -> (u16, Value) {
    let body = json
        .as_ref()
        .map(|v| serde_json::to_vec(v).expect("api: serializing the body"))
        .unwrap_or_default();
    let mut headers: Vec<(&str, &str)> = vec![("X-Meshtown-Token", token)];
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    let resp = http_within(port, method, path, &headers, &body, Duration::from_secs(5));
    let value = if resp.body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&resp.body).unwrap_or(Value::Null)
    };
    (resp.status, value)
}
