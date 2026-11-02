//! Requests of the end-to-end sign-in tests (`core/tests/web_auth.rs`, `web_auth_link.rs`): token exchange, login,
//! requests with a cookie. Included as `#[path = "common/web_auth_session.rs"] mod session_common;` next to
//! `common/web_listener.rs` and `common/web_listener_resp.rs` (`mod resp_common;`), takes `http_within` and `Resp` from it.

use std::time::Duration;

use serde_json::{json, Value};

use super::resp_common::{http_within, Resp};

/// Response timeout: a hash of 600,000 iterations in a debug build under battery load takes long.
const SLOW: Duration = Duration::from_secs(60);

/// `common::http` with a 60 s timeout instead of 5: the end-to-end sign-in tests call the hashing endpoints.
pub(crate) fn http(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Resp {
    http_within(port, method, path, headers, body, SLOW)
}

/// The response body as JSON (empty — `Null`).
pub(crate) fn body_json(resp: &Resp) -> Value {
    serde_json::from_slice(&resp.body).unwrap_or(Value::Null)
}

/// A `name=value` pair from `Set-Cookie` — the same one that is sent back in `Cookie`.
fn cookie_pair(resp: &Resp) -> Option<String> {
    let headers = resp.headers();
    let line = headers.get("set-cookie")?;
    let pair = line.split(';').next()?.trim();
    let value = pair.split_once('=')?.1;
    (!value.is_empty()).then(|| pair.to_string())
}

fn post_json(port: u16, path: &str, body: &Value) -> Resp {
    let bytes = serde_json::to_vec(body).expect("body");
    http(
        port,
        "POST",
        path,
        &[("Content-Type", "application/json")],
        &bytes,
    )
}

/// `POST /api/auth/token`: the status and the session cookie (if issued).
pub(crate) fn exchange(port: u16, token: &str) -> (u16, Option<String>) {
    let resp = post_json(port, "/api/auth/token", &json!({"token": token}));
    (resp.status, cookie_pair(&resp))
}

/// `POST /api/auth/login`: the whole response and the session cookie.
pub(crate) fn login(port: u16, password: &str) -> (Resp, Option<String>) {
    let resp = post_json(port, "/api/auth/login", &json!({"password": password}));
    let cookie = cookie_pair(&resp);
    (resp, cookie)
}

/// A request with the session cookie instead of the token.
pub(crate) fn with_cookie(
    port: u16,
    cookie: &str,
    method: &str,
    path: &str,
    json: Option<Value>,
) -> Resp {
    let body = json
        .map(|v| serde_json::to_vec(&v).expect("body"))
        .unwrap_or_default();
    let mut headers = vec![("Cookie", cookie)];
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    http(port, method, path, &headers, &body)
}
