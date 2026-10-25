//! Node actions: exit search, check of a domain/all domains, stop, restart.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::config::config_file::ProcessSettings;
use crate::web_gui::{ActionError, App, SlotHandle};

use super::helpers::{action_error, body_obj, map_string_field};
use super::slot_param::SlotParam;

pub(super) async fn action_find_exit_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    let map = match body_obj(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let domain = match map_string_field(&map, "domain") {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match slot.node.find_exit(&domain) {
        Ok(()) => (
            StatusCode::ACCEPTED,
            axum::Json(json!({"domain": domain, "added": true})),
        )
            .into_response(),
        Err(e) => action_error(e),
    }
}

pub(super) async fn action_check_domain_handler(
    SlotParam(slot): SlotParam,
    bytes: Bytes,
) -> Response {
    let map = match body_obj(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let domain = match map_string_field(&map, "domain") {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match slot.node.check_domain(&domain) {
        Ok(()) => (StatusCode::ACCEPTED, axum::Json(json!({"domain": domain}))).into_response(),
        Err(e) => action_error(e),
    }
}

pub(super) async fn action_check_all_handler(SlotParam(slot): SlotParam) -> Response {
    match slot.node.check_all() {
        Ok(n) => (StatusCode::ACCEPTED, axum::Json(json!({"queued": n}))).into_response(),
        Err(e) => action_error(e),
    }
}

pub(super) async fn action_stop_handler(State(app): State<Arc<App>>) -> Response {
    app.stopping.store(true, Ordering::SeqCst);
    app.host.stop_all();
    (StatusCode::ACCEPTED, axum::Json(json!({"stopping": true}))).into_response()
}

pub(super) async fn action_restart_handler(
    State(app): State<Arc<App>>,
    SlotParam(slot): SlotParam,
) -> Response {
    let outcome = match slot.node.restart() {
        Err(ActionError::NoNetwork(text)) => restart_without_network(&app, &slot, text).await,
        other => other,
    };
    match outcome {
        Ok(()) => (StatusCode::ACCEPTED, axum::Json(json!({"restart": true}))).into_response(),
        Err(e) => action_error(e),
    }
}

/// A restart of the primary slot that has no network: the only thing it can restart is the process-wide
/// proxy, which takes the pending `socks.*` settings. Anything else keeps the original refusal.
async fn restart_without_network(
    app: &App,
    slot: &SlotHandle,
    refusal: String,
) -> Result<(), ActionError> {
    let refused = ActionError::NoNetwork(refusal);
    let Some(front) = app.host.proxy_front().filter(|_| slot.id.carries_process()) else {
        return Err(refused);
    };
    let Some(process) = pending_socks_process(slot) else {
        return Err(refused);
    };
    front
        .apply(&process)
        .await
        .map_err(|e| ActionError::Busy(format!("{e:#}")))?;
    let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.settle_socks_baseline(&process);
    Ok(())
}

/// The process settings to apply when `socks.*` waits for a restart and no launch flag owns it.
fn pending_socks_process(slot: &SlotHandle) -> Option<ProcessSettings> {
    let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    let pending = cfg.pending_restart_paths();
    let waits = pending.iter().any(|p| p.starts_with("socks."));
    let flagged = cfg
        .cli_override_paths
        .iter()
        .any(|p| p.starts_with("socks."));
    (waits && !flagged).then(|| cfg.file_settings.process.clone())
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;
