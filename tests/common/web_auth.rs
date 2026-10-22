//! Helpers for the end-to-end sign-in tests (`tests/web_auth.rs`): token exchange, requests with a cookie, a password
//! file in the on-disk format with a small number of iterations. Included next to
//! `common/web_listener.rs`, takes `http` and `Resp` from it.
#![allow(dead_code)] // each test crate takes its own subset

use std::num::NonZeroU32;
use std::path::Path;

use serde_json::{json, Value};

use std::time::Duration;

use super::common::{http_within, Resp};

/// Response timeout: a hash of 600,000 iterations in a debug build under battery load takes long.
const SLOW: Duration = Duration::from_secs(60);

/// `common::http` with a 60 s timeout instead of 5: the end-to-end sign-in tests call the hashing endpoints.
pub(crate) fn http(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Resp {
    http_within(port, method, path, headers, body, SLOW)
}

pub(crate) const PASSWORD_OK: &str = "correct horse battery";

/// The response body as JSON (empty — `Null`).
pub(crate) fn body_json(resp: &Resp) -> Value {
    serde_json::from_slice(&resp.body).unwrap_or(Value::Null)
}

/// A `name=value` pair from `Set-Cookie` — the same one that is sent back in `Cookie`.
fn cookie_pair(resp: &Resp) -> Option<String> {
    let line = resp.headers.get("set-cookie")?;
    let pair = line.split(';').next()?.trim();
    let value = pair.split_once('=')?.1;
    (!value.is_empty()).then(|| pair.to_string())
}

fn post_json(port: u16, path: &str, body: &Value) -> Resp {
    let bytes = serde_json::to_vec(body).expect("body");
    http(port, "POST", path, &[("Content-Type", "application/json")], &bytes)
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
pub(crate) fn with_cookie(port: u16, cookie: &str, method: &str, path: &str, json: Option<Value>) -> Resp {
    let body = json.map(|v| serde_json::to_vec(&v).expect("body")).unwrap_or_default();
    let mut headers = vec![("Cookie", cookie)];
    if !body.is_empty() {
        headers.push(("Content-Type", "application/json"));
    }
    http(port, method, path, &headers, &body)
}

/// The permanent token from the `web-token` file.
pub(crate) fn file_token(dir: &Path) -> String {
    let text = std::fs::read_to_string(dir.join("web-token")).expect("web-token");
    text.trim().to_string()
}

/// A temporary token from the entry URL (`…/?token=T`).
pub(crate) fn temp_token(url: &str) -> String {
    url.split_once("?token=").expect("no token in the URL").1.to_string()
}

/// The `web-password` file in the on-disk format (0600); the hash is computed by `ring` right here.
pub(crate) fn write_password(dir: &Path, password: &str, iterations: u32) {
    let salt = [7u8; 16];
    let mut hash = [0u8; 32];
    let n = NonZeroU32::new(iterations).expect("iterations");
    ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA256, n, &salt, password.as_bytes(), &mut hash);
    let b64 = |b: &[u8]| data_encoding::BASE64.encode(b);
    let doc = json!({
        "version": 1, "alg": "pbkdf2-sha256", "iterations": iterations,
        "salt": b64(&salt), "hash": b64(&hash), "set_at": 1_700_000_000u64,
    });
    let path = dir.join("web-password");
    std::fs::write(&path, format!("{doc}\n")).expect("writing web-password");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("permissions");
    }
}
