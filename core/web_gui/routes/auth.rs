//! Sign-in handlers `/api/auth/*`: status, token exchange, password sign-in, sign-out, password reset. Password
//! change is in `auth_password.rs`. Order of checks: limiter → `hash_gate` permit →
//! limiter again → check → `record_fail`/`clear_fails`; the `AuthState` lock across `await`
//! is not held.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use crate::web_gui::auth::password::{self, PasswordVerdict};
use crate::web_gui::auth::password_file::{self, PasswordFileState};
use crate::web_gui::auth::tickets::Via;
use crate::web_gui::guard::{ct_eq, Credential};
use crate::web_gui::http::{json_err, ConnInfo};
use crate::web_gui::{cookie, App};

mod token;
pub(super) use token::{status_handler, token_handler};

/// How long a temporary token issued by `reset` lives (seconds, like `TEMP_TTL`).
const RESET_EXPIRES_SEC: u64 = 600;

/// A string field of the body: no body, not JSON or no such field — 400 `bad_request`.
pub(super) fn body_string_field(bytes: &[u8], name: &str) -> Result<String, Response> {
    let value: Option<Value> = serde_json::from_slice(bytes).ok();
    let found = value.and_then(|v| v.get(name)?.as_str().map(str::to_string));
    found.ok_or_else(|| {
        let text = format!("a string field {name} is required");
        json_err(StatusCode::BAD_REQUEST, "bad_request", &text)
    })
}

fn too_many(wait: Duration) -> Response {
    let secs = wait.as_secs_f64().ceil().max(1.0) as u64;
    let mut resp = json_err(
        StatusCode::TOO_MANY_REQUESTS,
        "too_many_attempts",
        "too many failed attempts, wait",
    );
    resp.headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from(secs));
    resp
}

/// The limiter: `Err` is a ready 429, the check does not start.
fn reject_if_blocked(app: &App, ip: IpAddr) -> Result<(), Response> {
    let limiter = app
        .shared
        .auth
        .limiter
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    limiter
        .check_not_blocked(ip, Instant::now())
        .map_err(too_many)
}

fn record_attempt(app: &App, ip: IpAddr, success: bool) {
    let mut limiter = app
        .shared
        .auth
        .limiter
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match success {
        true => limiter.clear_fails(ip),
        false => limiter.record_fail(ip, Instant::now()),
    }
}

fn internal_error_response(text: &str) -> Response {
    json_err(StatusCode::INTERNAL_SERVER_ERROR, "internal", text)
}

/// Permit for one heavy hash job.
pub(super) async fn acquire_hash_permit(
    app: &App,
) -> Result<tokio::sync::SemaphorePermit<'_>, String> {
    let permit = app.shared.auth.hash_gate.acquire().await;
    permit.map_err(|_| "check unavailable".to_string())
}

/// Permit for one check, and a repeated limiter check already under it.
async fn acquire_attempt_permit(
    app: &App,
    ip: IpAddr,
) -> Result<tokio::sync::SemaphorePermit<'_>, Response> {
    reject_if_blocked(app, ip)?;
    let permit = acquire_hash_permit(app)
        .await
        .map_err(|e| internal_error_response(&e))?;
    reject_if_blocked(app, ip)?;
    Ok(permit)
}

/// Opens a session and returns `{"via"}` with `Set-Cookie`.
fn open_session_response(app: &App, via: Via) -> Response {
    let opened = {
        let mut tickets = app
            .shared
            .auth
            .tickets
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tickets.open_session(via, Instant::now())
    };
    match opened {
        Err(e) => internal_error_response(&format!("session not opened: {e}")),
        Ok(id) => {
            let cookie = cookie::session_cookie_header(&app.shared.auth.cookie_name, &id);
            let body = json!({"via": via.as_str()});
            ([(header::SET_COOKIE, cookie)], axum::Json(body)).into_response()
        }
    }
}

fn no_password() -> Response {
    json_err(StatusCode::CONFLICT, "no_password", "no password set")
}

/// A saved record or a ready 409 (no directory, no file, or it can't be read).
fn stored_record(app: &App) -> Result<password::PasswordRecord, Response> {
    let Some(dir) = &app.shared.auth.password_dir else {
        return Err(no_password());
    };
    match password_file::read_password_file(dir) {
        PasswordFileState::Set(rec) => Ok(rec),
        _ => Err(no_password()),
    }
}

/// Rehash of a record with an old iteration count; an error becomes a log line.
async fn rehash(app: &App, pw: String) {
    let Some(dir) = app.shared.auth.password_dir.clone() else {
        return;
    };
    let work = move || {
        password::hash(&pw)
            .and_then(|r| password_file::write_password_file(&dir, &r).map_err(|e| e.to_string()))
    };
    let done = crate::node::output::spawn_blocking(work).await;
    let failed = done.map_err(|e| e.to_string()).and_then(|r| r);
    if let Err(e) = failed {
        crate::node::output::say("web", format!("password not rehashed: {e}"));
    }
}

pub(super) async fn login_handler(
    State(app): State<Arc<App>>,
    Extension(conn): Extension<ConnInfo>,
    body: Bytes,
) -> Response {
    let pw = match body_string_field(&body, "password") {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let ip = conn.client_addr.ip();
    let _permit = match acquire_attempt_permit(&app, ip).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let rec = match stored_record(&app) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let again = pw.clone();
    let verdict = crate::node::output::spawn_blocking(move || password::verify(&rec, &again)).await;
    let Ok(verdict) = verdict else {
        return internal_error_response("password check did not finish");
    };
    record_attempt(&app, ip, verdict != PasswordVerdict::Mismatch);
    finish_login(&app, verdict, pw).await
}

async fn finish_login(app: &App, verdict: PasswordVerdict, pw: String) -> Response {
    if verdict == PasswordVerdict::Mismatch {
        return json_err(
            StatusCode::UNAUTHORIZED,
            "bad_password",
            "password not accepted",
        );
    }
    if verdict == PasswordVerdict::MatchNeedsRehash {
        rehash(app, pw).await;
    }
    open_session_response(app, Via::Password)
}

pub(super) async fn logout_handler(
    State(app): State<Arc<App>>,
    cred: Option<Extension<Credential>>,
) -> Response {
    let session = cred.and_then(|Extension(c)| c.session_id);
    if let Some(id) = session {
        let mut tickets = app
            .shared
            .auth
            .tickets
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tickets.close_session(&id);
    }
    let cleared = cookie::expired_session_cookie_header(&app.shared.auth.cookie_name);
    ([(header::SET_COOKIE, cleared)], axum::Json(json!({}))).into_response()
}

/// Only the `X-Meshtown-Token` header with the permanent token: a session cookie and `?token=` don't qualify.
fn header_token_matches(app: &App, headers: &HeaderMap) -> bool {
    let sent = headers
        .get("x-meshtown-token")
        .and_then(|v| v.to_str().ok());
    let real = app.shared.token();
    sent.is_some_and(|s| ct_eq(s.as_bytes(), real.as_bytes()))
}

pub(super) async fn reset_handler(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    if !header_token_matches(&app, &headers) {
        let text = "password reset only with the permanent token in the header";
        return json_err(StatusCode::FORBIDDEN, "token_only", text);
    }
    if let Some(dir) = &app.shared.auth.password_dir {
        if let Err(e) = password_file::remove_password_file(dir) {
            return internal_error_response(&format!("password file not deleted: {e}"));
        }
    }
    let mut tickets = app
        .shared
        .auth
        .tickets
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    tickets.close_sessions_except(None);
    tickets.drop_temp_tokens();
    drop(tickets);
    let body = json!({"urls": app.shared.issue_entry_urls(), "expires_sec": RESET_EXPIRES_SEC});
    (StatusCode::OK, axum::Json(body)).into_response()
}
