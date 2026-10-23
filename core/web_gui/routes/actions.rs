//! Node actions: exit search, check of a domain/all domains, stop, restart.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::web_gui::App;

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

pub(super) async fn action_restart_handler(SlotParam(slot): SlotParam) -> Response {
    match slot.node.restart() {
        Ok(()) => (StatusCode::ACCEPTED, axum::Json(json!({"restart": true}))).into_response(),
        Err(e) => action_error(e),
    }
}
