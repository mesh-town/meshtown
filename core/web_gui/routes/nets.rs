//! `/api/nets`: the networks of the process and the way to add one.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::helpers::body_obj;
use super::state_host::API;
use crate::node::host::MAX_NETS;
use crate::web_gui::http::json_err;
use crate::web_gui::net_brief::briefs;
use crate::web_gui::{App, HostError};

pub(super) async fn nets_get_handler(State(app): State<Arc<App>>) -> Response {
    let nets = briefs(app.host.as_ref());
    (
        StatusCode::OK,
        axum::Json(json!({"api": API, "nets": nets})),
    )
        .into_response()
}

/// Adds a slot waiting for a network key: 201 for a new one, 200 when an empty slot already waited.
pub(super) async fn nets_post_handler(State(app): State<Arc<App>>, bytes: Bytes) -> Response {
    if let Err(resp) = body_obj(&bytes) {
        return resp;
    }
    match app.host.add_slot() {
        Ok(added) => {
            let status = if added.fresh {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            let nets = briefs(app.host.as_ref());
            let body = json!({"slot": added.id.to_string(), "fresh": added.fresh, "nets": nets});
            (status, axum::Json(body)).into_response()
        }
        Err(e) => host_error_response(e),
    }
}

/// The reply for a failed host operation.
pub(crate) fn host_error_response(err: HostError) -> Response {
    match err {
        HostError::TooManyNets => {
            let message = format!("at most {MAX_NETS} networks on this computer");
            let body = json!({"error": "too_many_nets", "message": message, "limit": MAX_NETS});
            (StatusCode::CONFLICT, axum::Json(body)).into_response()
        }
        HostError::UnknownNet => json_err(
            StatusCode::NOT_FOUND,
            "unknown_net",
            "no such network on this computer",
        ),
        HostError::AlreadyAdded => json_err(
            StatusCode::CONFLICT,
            "net_already_added",
            "this network is already on this computer",
        ),
        HostError::Busy(message) => json_err(StatusCode::CONFLICT, "busy", &message),
        HostError::Io(message) => json_err(StatusCode::INTERNAL_SERVER_ERROR, "internal", &message),
    }
}

#[cfg(test)]
#[path = "nets_tests.rs"]
mod tests;
