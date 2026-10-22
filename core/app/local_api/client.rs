//! Thin wrappers over the chain `request_bytes_without_body` → `exchange` → `reply_from_response`. There is no retry on 401 and
//! no token re-read here — that is the `tray_role` poller's job; `dead_code` on both functions in
//! a normal build is expected and allowed here until the poller uses them.

use super::classify::reply_from_response;
use super::request::{request_bytes_without_body, request_bytes_put};
use super::transport::{exchange, exchange_within, INVITE_EXCHANGE_TIMEOUT};
use super::types::{ApiError, Reply};

/// Node state snapshot (`GET /api/state`). A wrapper over `request_bytes_without_body` → exchange →
/// `parse_response` → `reply_from_response`.
pub(crate) async fn get_state(port: u16, token: &str) -> Result<Reply, ApiError> {
    let request = request_bytes_without_body("GET", "/api/state", port, token);
    let (status, body) = exchange(&request, port).await?;
    Ok(reply_from_response(status, &body))
}

/// Node stop (`POST /api/actions/stop`). The same chain as `get_state`.
pub(crate) async fn post_stop(port: u16, token: &str) -> Result<Reply, ApiError> {
    let request = request_bytes_without_body("POST", "/api/actions/stop", port, token);
    let (status, body) = exchange(&request, port).await?;
    Ok(reply_from_response(status, &body))
}

/// The current configuration document (`GET /api/config`). The same chain as `get_state`.
pub(crate) async fn get_config(port: u16, token: &str) -> Result<Reply, ApiError> {
    let request = request_bytes_without_body("GET", "/api/config", port, token);
    let (status, body) = exchange(&request, port).await?;
    Ok(reply_from_response(status, &body))
}

/// The node's invite (`GET /api/network/invite`). The same chain as `get_config`. The response body
/// carries a secret — the caller extracts the string at once and never formats it anywhere.
pub(crate) async fn get_invite(port: u16, token: &str) -> Result<Reply, ApiError> {
    let request = request_bytes_without_body("GET", "/api/network/invite", port, token);
    let (status, body) = exchange_within(&request, port, INVITE_EXCHANGE_TIMEOUT).await?;
    Ok(reply_from_response(status, &body))
}

/// A configuration edit (`PUT /api/config`). `body` is an already serialized document
/// `{"rev": <rev>, "config": {...}}` (the server merges the patch itself, no need to send the full document).
pub(crate) async fn put_config(port: u16, token: &str, body: &[u8]) -> Result<Reply, ApiError> {
    let request = request_bytes_put("/api/config", port, token, body);
    let (status, response_body) = exchange(&request, port).await?;
    Ok(reply_from_response(status, &response_body))
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "client_invite_tests.rs"]
mod invite_tests;
