//! Guard for every request: peer → absolute-form target → `Host` → `Origin` → credentials.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::request::Parts;
use axum::http::{Method, StatusCode, Uri};
use axum::response::Response;

use super::auth::access::is_client_allowed;
use super::auth::iface_os::current_view;
use super::auth::tickets::Via;
use super::host_name::{is_loopback_client, is_mesh_host_name};
use super::http::{host_of, json_err, query_param, ConnInfo};
use super::{cookie, App, WebShared};
use crate::config::config_file::ListenAccess;
use crate::node::output::NET_NAME;

/// What the request used to prove the right to enter; put into the request extensions.
#[derive(Clone, Debug)]
pub(crate) struct Credential {
    pub via: Via,
    /// Session id from the cookie; `None` for the permanent token.
    pub session_id: Option<String>,
}

/// `/api/*` paths open without credentials (exact comparison).
const OPEN_API: [&str; 3] = ["/api/auth/status", "/api/auth/token", "/api/auth/login"];

/// Checks go strictly in order; the first mismatch returns a ready error. `Ok(Some)` —
/// the request is confirmed, `Ok(None)` — let through without credentials (static files, open sign-in handlers).
pub(crate) fn check_request(
    req: &Parts,
    conn: &ConnInfo,
    shared: &WebShared,
) -> Result<Option<Credential>, Response> {
    check_client(conn, shared)?;
    let host = host_of(conn.local.ip(), conn.local.port());
    check_absolute_form(&req.uri, &host)?;
    let host = check_host(req, &host, conn.client_addr.ip())?;
    check_origin(req, &host)?;
    check_credential(req, shared)
}

fn bad_client_response(mode: ListenAccess) -> Response {
    let place = match mode {
        ListenAccess::Localhost => "this computer",
        ListenAccess::Localnet => "the local network",
    };
    let text = format!("the {NET_NAME} page is open only from {place}");
    json_err(StatusCode::FORBIDDEN, "bad_peer", &text)
}

fn check_client(conn: &ConnInfo, shared: &WebShared) -> Result<(), Response> {
    let mode = shared.auth.access();
    let iface_ok = shared.auth.iface_verdict(&current_view(), conn).allows();
    if is_client_allowed(mode, conn.client_addr.ip()) && iface_ok {
        return Ok(());
    }
    Err(bad_client_response(mode))
}

fn bad_host_response(host: &str) -> Response {
    let text = format!("the page opens only at {host}");
    json_err(StatusCode::FORBIDDEN, "bad_host", &text)
}

/// Absolute-form target (`GET http://... HTTP/1.1`) — immediately a "foreign host", before `Host`.
fn check_absolute_form(uri: &Uri, host: &str) -> Result<(), Response> {
    if uri.authority().is_some() {
        return Err(bad_host_response(host));
    }
    Ok(())
}

/// The `Host` header — exactly one value: the literal of the socket's local address or, for a loopback
/// client only, a `<label>.meshtown[:<port>]` name. Returns the accepted value.
fn check_host(req: &Parts, host: &str, client: IpAddr) -> Result<String, Response> {
    let mut values = req.headers.get_all(axum::http::header::HOST).iter();
    let first = values.next().and_then(|v| v.to_str().ok());
    let single = values.next().is_none();
    match first {
        Some(value) if single && value == host => Ok(host.to_string()),
        Some(value) if single && is_mesh_name_for(value, client) => Ok(value.to_string()),
        _ => Err(bad_host_response(host)),
    }
}

fn is_mesh_name_for(value: &str, client: IpAddr) -> bool {
    is_loopback_client(client) && is_mesh_host_name(value)
}

/// `Origin` is required on all methods except `GET`/`HEAD`; when present it equals `http://<Host>`
/// for the `Host` accepted by `check_host`.
fn check_origin(req: &Parts, host: &str) -> Result<(), Response> {
    let origin = req
        .headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    let reading = matches!(req.method, Method::GET | Method::HEAD);
    let ok = match origin {
        Some(value) => value.strip_prefix("http://") == Some(host),
        None => reading,
    };
    if ok {
        return Ok(());
    }
    let text = format!("the request didn't come from the {NET_NAME} page");
    Err(json_err(StatusCode::FORBIDDEN, "bad_origin", &text))
}

fn check_credential(req: &Parts, shared: &WebShared) -> Result<Option<Credential>, Response> {
    let cred = find_credential(req, shared);
    let path = req.uri.path();
    if cred.is_some() || !path.starts_with("/api/") || OPEN_API.contains(&path) {
        return Ok(cred);
    }
    let text = "no token or the wrong token";
    Err(json_err(StatusCode::UNAUTHORIZED, "bad_token", text))
}

/// A session from the cookie, otherwise the permanent token (header, then `?token=`). The temporary token
/// doesn't pass here: only the exchange handler accepts it.
fn find_credential(req: &Parts, shared: &WebShared) -> Option<Credential> {
    session_credential(req, shared).or_else(|| token_credential(req, shared))
}

fn session_credential(req: &Parts, shared: &WebShared) -> Option<Credential> {
    let auth = &shared.auth;
    let id = cookie::session_id(&req.headers, &auth.cookie_name)?;
    let tickets = auth.tickets.lock().unwrap_or_else(|e| e.into_inner());
    let via = tickets.session(&id, Instant::now())?;
    Some(Credential {
        via,
        session_id: Some(id),
    })
}

fn token_credential(req: &Parts, shared: &WebShared) -> Option<Credential> {
    let from_header = req
        .headers
        .get("x-meshtown-token")
        .and_then(|v| v.to_str().ok());
    let candidate = from_header
        .map(str::to_string)
        .or_else(|| query_param(&req.uri, "token"))?;
    let real = shared.token();
    ct_eq(candidate.as_bytes(), real.as_bytes()).then_some(Credential {
        via: Via::PermanentToken,
        session_id: None,
    })
}

pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

/// Admits a request: without `ConnInfo` — 403 `bad_peer` (closed by default), otherwise the checks and
/// `Credential` in the extensions.
pub(crate) fn admit_request(req: Request, shared: &WebShared) -> Result<Request, Response> {
    let Some(conn) = req.extensions().get::<ConnInfo>().copied() else {
        return Err(bad_client_response(shared.auth.access()));
    };
    let (mut parts, body) = req.into_parts();
    if let Some(cred) = check_request(&parts, &conn, shared)? {
        parts.extensions.insert(cred);
    }
    Ok(Request::from_parts(parts, body))
}

pub(crate) async fn guard_middleware(
    State(app): State<Arc<App>>,
    req: Request,
    next: axum::middleware::Next,
) -> Response {
    match admit_request(req, &app.shared) {
        Ok(req) => next.run(req).await,
        Err(resp) => resp,
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "guard_session_tests.rs"]
mod session_tests;

#[cfg(test)]
#[path = "guard_mesh_name_tests.rs"]
mod mesh_name_tests;
