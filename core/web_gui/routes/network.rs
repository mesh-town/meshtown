//! Joining a network and creating a new one: `/api/network/join`, `/api/network/create`.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::node::output::NET_NAME;
use crate::web_gui::http::json_err;
use crate::web_gui::{App, SaveKey};

use super::helpers::{action_error, body_obj, invalid_key_response, map_string_field};

/// The "keep port publications" choice from the request body: `true` only for JSON `true`; a missing field,
/// a non-bool and `false` mean "No".
fn wants_keep_ports(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.get("keep_ports") == Some(&serde_json::Value::Bool(true))
}

/// Sign-in by an invite string: the response is immediate, waiting for members happens on the node (`node.invite_join`).
/// The node writes the received key to the settings file with the `save` closure and joins the network itself.
fn join_by_invite_response(app: &Arc<App>, text: &str, keep_ports: bool) -> Response {
    let parsed = match crate::invite::parse(text) {
        Ok(p) => p,
        Err(e) => return invalid_key_response(&e, &format!("{e:#}")),
    };
    if let Err(e) = crate::invite::check_fresh(&parsed.head, crate::web_gui::now_unix()) {
        return invalid_key_response(&e, &format!("{e:#}"));
    }
    let label = parsed.head.label.clone();
    let owner = app.clone();
    let save: SaveKey = Box::new(move |key| {
        let mut cfg = owner.config.lock().unwrap_or_else(|e| e.into_inner());
        cfg.save_network_key(key, keep_ports)
            .map_err(|_| "settings file not written".to_string())
    });
    if let Err(e) = app.node.join_by_invite(parsed.head, parsed.peers, save) {
        return action_error(e);
    }
    (
        StatusCode::ACCEPTED,
        axum::Json(json!({"label": label, "restart": false, "waiting": true})),
    )
        .into_response()
}

pub(super) async fn network_join_handler(State(app): State<Arc<App>>, bytes: Bytes) -> Response {
    let map = match body_obj(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let key_or_invite = match map_string_field(&map, "key") {
        Ok(k) => k,
        Err(resp) => return resp,
    };
    let keep_ports = wants_keep_ports(&map);
    if crate::invite::looks_like_invite(&key_or_invite) {
        return join_by_invite_response(&app, &key_or_invite, keep_ports);
    }
    let input = match crate::members::network_key::parse_user_input(&key_or_invite) {
        Ok(i) => i,
        Err(e) => {
            return invalid_key_response(
                &e,
                "This doesn't look like an invite. Copy it from the network member again in full and paste it here.",
            )
        }
    };
    let label = {
        let mut cfg = app.config.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(resp) = cfg.save_network_key(&key_or_invite, keep_ports) {
            return resp;
        }
        input.label.clone()
    };
    app.node.switch_network(&key_or_invite);
    (
        StatusCode::ACCEPTED,
        axum::Json(json!({"label": label, "restart": true})),
    )
        .into_response()
}

pub(super) async fn network_create_handler(State(app): State<Arc<App>>, bytes: Bytes) -> Response {
    let map = match body_obj(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let label_input = match map_string_field(&map, "label") {
        Ok(l) => l,
        Err(resp) => return resp,
    };
    let keep_ports = wants_keep_ports(&map);
    let label = match crate::members::network_key::normalize_label(&label_input) {
        Ok(l) => l,
        Err(e) => {
            let body = json!({
                "error": "invalid",
                "message": "this name doesn't fit",
                "fields": [{"path": "label", "message": e.to_string()}],
            });
            return (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response();
        }
    };
    let tail = match crate::members::words::generate_tail() {
        Ok(t) => t,
        Err(e) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                &format!("couldn't create network {NET_NAME}: {e}"),
            )
        }
    };
    let key = crate::members::network_key::format_key_string(Some(&label), &tail);
    {
        let mut cfg = app.config.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(resp) = cfg.save_network_key(&key, keep_ports) {
            return resp;
        }
    }
    app.node.switch_network(&key);
    (
        StatusCode::OK,
        axum::Json(json!({"key": key, "label": label, "restart": true})),
    )
        .into_response()
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;
