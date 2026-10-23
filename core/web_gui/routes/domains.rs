//! Domains: `/api/domains` (adding and removing).

use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::web_gui::http::{json_err, query_param};

use super::helpers::{action_error, body_obj, map_string_field};
use super::slot_param::SlotParam;

pub(super) async fn domains_post_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
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
    SlotParam(slot): SlotParam,
    uri: axum::http::Uri,
) -> Response {
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
