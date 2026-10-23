//! Talking to the node over HTTP and the start policy: one request with a retry on
//! 401, the "spawn / attach / port in use" decision, and the form of a reply for the `[tray] …` lines.

use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

use crate::gui::tray::app::local_api::{self, ApiError, Reply};

pub(super) const KILLED_BY_SIGNAL_TEXT: &str = "signal";

/// The outcome of the start policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StartDecision {
    Spawn,
    Attach,
    PortBusy,
}

/// The single place for the "should the node be started" policy: startup, "Start Mesh Town", polling with no node.
pub(super) fn start_policy(reply: &Result<Reply, ApiError>) -> StartDecision {
    match reply {
        Err(ApiError::Refused) => StartDecision::Spawn,
        Ok(Reply::Ok(_)) => StartDecision::Attach,
        Ok(Reply::Unauthorized)
        | Ok(Reply::Accepted)
        | Ok(Reply::Stopping)
        | Ok(Reply::OtherStatus(_))
        | Err(ApiError::Timeout)
        | Err(ApiError::NotHttp(_)) => StartDecision::PortBusy,
    }
}

/// The form of a reply without its body — for the `[tray] …` lines.
pub(super) fn reply_summary(reply: &Result<Reply, ApiError>) -> String {
    match reply {
        Ok(Reply::Ok(_)) => "HTTP 200".to_string(),
        Ok(Reply::Accepted) => "HTTP 202".to_string(),
        Ok(Reply::Unauthorized) => "HTTP 401".to_string(),
        Ok(Reply::Stopping) => "HTTP 503 stopping".to_string(),
        Ok(Reply::OtherStatus(code)) => format!("HTTP {code}"),
        Err(ApiError::Refused) => "connection refused".to_string(),
        Err(ApiError::Timeout) => "no reply in time".to_string(),
        Err(ApiError::NotHttp(why)) => format!("not HTTP: {why}"),
    }
}

pub(super) fn child_status_text(status: &ExitStatus) -> String {
    status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| KILLED_BY_SIGNAL_TEXT.to_string())
}

#[derive(Clone, Copy)]
pub(super) enum NodeRequest<'a> {
    State,
    Overview,
    Stop,
    Invite(Option<&'a str>),
}

/// A request with the token read from the file right now; on 401 it re-reads the token and retries once at once. `total_timeout` is
/// a deadline over both calls (the remainder of the "Stop" procedure); when it expires the result is `Timeout`.
pub(super) async fn fetch_reply(
    port: u16,
    token_path: &Path,
    api: NodeRequest<'_>,
    total_timeout: Option<Duration>,
) -> Result<Reply, ApiError> {
    let call = async {
        let first = call_api(api, port, &super::run::read_token(token_path)).await;
        if matches!(first, Ok(Reply::Unauthorized)) {
            call_api(api, port, &super::run::read_token(token_path)).await
        } else {
            first
        }
    };
    match total_timeout {
        Some(total_timeout) => tokio::time::timeout(total_timeout, call)
            .await
            .unwrap_or(Err(ApiError::Timeout)),
        None => call.await,
    }
}

async fn call_api(api: NodeRequest<'_>, port: u16, token: &str) -> Result<Reply, ApiError> {
    match api {
        NodeRequest::State => local_api::get_state(port, token).await,
        NodeRequest::Overview => local_api::get_overview(port, token).await,
        NodeRequest::Stop => local_api::post_stop(port, token).await,
        NodeRequest::Invite(slot) => local_api::get_invite(port, token, slot).await,
    }
}

#[cfg(test)]
#[path = "api_tests.rs"]
mod tests;
