//! Domains: `/api/domains` (adding and removing).

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::web_gui::http::{json_err, query_param};
use crate::web_gui::App;

use super::helpers::{action_error, body_obj, map_string_field};

pub(super) async fn domains_post_handler(State(app): State<Arc<App>>, bytes: Bytes) -> Response {
    let slot = app.host.main_slot();
    let map = match body_obj(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let domain = match map_string_field(&map, "domain") {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match slot.node.add_domain(&domain) {
        Ok(name) => (StatusCode::OK, axum::Json(json!({"domain": name}))).into_response(),
        Err(e) => action_error(e),
    }
}

pub(super) async fn domains_delete_handler(
    State(app): State<Arc<App>>,
    uri: axum::http::Uri,
) -> Response {
    let slot = app.host.main_slot();
    let name = match query_param(&uri, "name") {
        Some(n) if !n.is_empty() => n,
        _ => {
            return json_err(
                StatusCode::BAD_REQUEST,
                "bad_request",
                "parameter name is required",
            )
        }
    };
    match slot.node.remove_domain(&name) {
        Ok(name) => (StatusCode::OK, axum::Json(json!({"domain": name}))).into_response(),
        Err(e) => action_error(e),
    }
}
