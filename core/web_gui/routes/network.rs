//! Joining a network and creating a new one: `/api/network/join`, `/api/network/create`.

use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::config::config_file::NetSlotId;
use crate::members::network_key::format_managed_key;
use crate::node::output::NET_NAME;
use crate::transport::net::PeerId;
use crate::web_gui::http::json_err;
use crate::web_gui::net_brief::briefs;
use crate::web_gui::{App, HostControl, SlotHandle};

use super::helpers::{body_obj, invalid_key_response, map_string_field};
use super::network_invite::{join_by_invite_response, label_of, net_already_added};
use super::slot_param::SlotParam;

/// Serializes "is this network already on the node" with the write of the key, so two tabs adopting the
/// same network cannot both pass the check. Taken before any slot's config lock and never across an `await`.
pub(super) static ADOPT: Mutex<()> = Mutex::new(());

/// The slot other than `except` that works with the network `id_hex`.
pub(crate) fn slot_with_network(
    host: &dyn HostControl,
    id_hex: &str,
    except: &NetSlotId,
) -> Option<NetSlotId> {
    let other = briefs(host)
        .into_iter()
        .filter(|b| b.id != except.to_string())
        .find(|b| b.id_hex.as_deref() == Some(id_hex))?;
    NetSlotId::parse(&other.id)
}

/// Whether the slot has a network, and the id of that network when it can be read.
fn own_network(slot: &SlotHandle) -> (bool, Option<String>) {
    let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    let id_hex = cfg
        .network_block()
        .get("id_hex")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    (cfg.current_network_key.is_some(), id_hex)
}

fn network_present() -> Response {
    json_err(
        StatusCode::CONFLICT,
        "network_present",
        "this tab already has a network: leave it first, or add a network",
    )
}

/// Refuses a network that does not fit the slot: the slot already works with another one, or another slot
/// has the network. `incoming` is the id of the network being joined, `None` for a new one.
pub(super) fn check_target(
    host: &dyn HostControl,
    slot: &SlotHandle,
    incoming: Option<&str>,
) -> Result<(), Response> {
    let (has_network, own_id) = own_network(slot);
    if has_network && (incoming.is_none() || incoming != own_id.as_deref()) {
        return Err(network_present());
    }
    let taken = incoming.and_then(|id_hex| slot_with_network(host, id_hex, &slot.id));
    match taken {
        Some(other) => Err(net_already_added(&other, label_of(host, &other))),
        None => Ok(()),
    }
}

/// Checks the slot and writes the key under `ADOPT`; no config lock is held while the check looks at other slots.
fn adopt_key(
    app: &App,
    slot: &SlotHandle,
    key: &str,
    incoming: Option<&str>,
    keep_ports: bool,
) -> Result<(), Response> {
    let _adopt = ADOPT.lock().unwrap_or_else(|e| e.into_inner());
    check_target(app.host.as_ref(), slot, incoming)?;
    let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.save_network_key(key, keep_ports)
}

/// Creates the key under `ADOPT` and writes it in the same step: `make` gets the node id of the secret the slot
/// works with, which is also the secret that goes into its settings file together with the key.
fn adopt_created(
    app: &App,
    slot: &SlotHandle,
    make: impl FnOnce(&PeerId) -> String,
    keep_ports: bool,
) -> Result<String, Response> {
    let _adopt = ADOPT.lock().unwrap_or_else(|e| e.into_inner());
    check_target(app.host.as_ref(), slot, None)?;
    let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    let key = make(&cfg.secret.peer_id());
    cfg.save_network_key(&key, keep_ports)?;
    Ok(key)
}

/// The key text of a new network: the words form with the creator's node id.
fn new_key_text(label: &str, tail: &str, root: &PeerId) -> String {
    format_managed_key(Some(label), tail, root)
}

/// The "keep port publications" choice from the request body: `true` only for JSON `true`; a missing field,
/// a non-bool and `false` mean "No".
fn wants_keep_ports(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.get("keep_ports") == Some(&serde_json::Value::Bool(true))
}

pub(super) async fn network_join_handler(
    State(app): State<Arc<App>>,
    SlotParam(slot): SlotParam,
    bytes: Bytes,
) -> Response {
    let (key_or_invite, keep_ports) = match join_request(&bytes) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    if crate::invite::looks_like_invite(&key_or_invite) {
        return join_by_invite_response(&app, &slot, &key_or_invite, keep_ports);
    }
    join_by_key_response(&app, &slot, &key_or_invite, keep_ports)
}

/// The `key` text and the "keep port publications" choice of a join request body.
pub(super) fn join_request(bytes: &Bytes) -> Result<(String, bool), Response> {
    let map = body_obj(bytes)?;
    let key_or_invite = map_string_field(&map, "key")?;
    Ok((key_or_invite, wants_keep_ports(&map)))
}

/// Sign-in by a words key: written to the slot at once, the node switches to it.
pub(super) fn join_by_key_response(
    app: &Arc<App>,
    slot: &SlotHandle,
    key_or_invite: &str,
    keep_ports: bool,
) -> Response {
    let input = match crate::members::network_key::parse_user_input(key_or_invite) {
        Ok(i) => i,
        Err(e) => {
            return invalid_key_response(
                &e,
                "This doesn't look like an invite. Copy it from the network member again in full and paste it here.",
            )
        }
    };
    let incoming = input.key.subkeys().network_id_hex();
    if let Err(resp) = adopt_key(app, slot, key_or_invite, Some(&incoming), keep_ports) {
        return resp;
    }
    let label = input.label.clone();
    slot.node.switch_network(key_or_invite);
    (
        StatusCode::ACCEPTED,
        axum::Json(json!({"label": label, "restart": true, "slot": slot.id.to_string()})),
    )
        .into_response()
}

pub(super) async fn network_create_handler(
    State(app): State<Arc<App>>,
    SlotParam(slot): SlotParam,
    bytes: Bytes,
) -> Response {
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
    let make = |root: &PeerId| new_key_text(&label, &tail, root);
    let key = match adopt_created(&app, &slot, make, keep_ports) {
        Ok(key) => key,
        Err(resp) => return resp,
    };
    slot.node.switch_network(&key);
    (
        StatusCode::OK,
        axum::Json(json!({"key": key, "label": label, "restart": true})),
    )
        .into_response()
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "network_present_tests.rs"]
mod present_tests;

#[cfg(test)]
#[path = "network_create_tests.rs"]
mod create_tests;

#[cfg(test)]
#[path = "join_key_tests.rs"]
mod join_key_tests;
