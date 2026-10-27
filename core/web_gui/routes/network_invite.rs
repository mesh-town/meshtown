//! Sign-in by an invite string and the "already on the node" answer.

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::config::config_file::NetSlotId;
use crate::web_gui::config_api::ConfigState;
use crate::web_gui::net_brief::briefs;
use crate::web_gui::{App, HostControl, SaveKey, SlotHandle};

use super::helpers::{action_error, invalid_key_response};
use super::network::{check_target, ADOPT};

/// The label of the network that the slot `id` works with.
pub(super) fn label_of(host: &dyn HostControl, id: &NetSlotId) -> Option<String> {
    let id = id.to_string();
    briefs(host).into_iter().find(|b| b.id == id)?.label
}

/// 409: the network is already on the node, in slot `other`.
pub(super) fn net_already_added(other: &NetSlotId, label: Option<String>) -> Response {
    let body = json!({
        "error": "net_already_added",
        "message": "this network is already on this computer",
        "slot": other.to_string(),
        "label": label,
    });
    (StatusCode::CONFLICT, axum::Json(body)).into_response()
}

/// Sign-in by an invite string: the response is immediate, waiting for members happens on the node (`node.invite_join`).
/// The node writes the received key to the settings file with the `save` closure and joins the network itself.
pub(super) fn join_by_invite_response(
    app: &Arc<App>,
    slot: &SlotHandle,
    text: &str,
    keep_ports: bool,
) -> Response {
    let parsed = match crate::invite::parse(text) {
        Ok(p) => p,
        Err(e) => return invalid_key_response(&e, &format!("{e:#}")),
    };
    if let Err(e) = crate::invite::check_fresh(&parsed.head, crate::web_gui::now_unix()) {
        return invalid_key_response(&e, &format!("{e:#}"));
    }
    let incoming = data_encoding::HEXLOWER.encode(&parsed.head.network_id);
    let adopt = ADOPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(resp) = check_target(app.host.as_ref(), slot, Some(&incoming)) {
        return resp;
    }
    drop(adopt);
    let label = parsed.head.label.clone();
    let config = slot.config.clone();
    let save = save_key_of(config, keep_ports);
    if let Err(e) = slot.node.join_by_invite(parsed.head, parsed.peers, save) {
        return action_error(e);
    }
    (
        StatusCode::ACCEPTED,
        axum::Json(
            json!({"label": label, "restart": false, "waiting": true, "slot": slot.id.to_string()}),
        ),
    )
        .into_response()
}

/// The closure that stores a received network key, and a new node key if the join changed it.
pub(super) fn save_key_of(config: Arc<Mutex<ConfigState>>, keep_ports: bool) -> SaveKey {
    Box::new(move |key, new_owner| {
        let mut cfg = config.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(secret) = new_owner {
            cfg.adopt_secret(secret.clone());
        }
        cfg.save_network_key(key, keep_ports)
            .map_err(|_| "settings file not written".to_string())
    })
}

#[cfg(test)]
#[path = "network_invite_tests.rs"]
mod tests;
