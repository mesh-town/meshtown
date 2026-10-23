//! `/api/config`: reading and writing the configuration through `ConfigState`.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::config::config_file::NetSlotId;
use crate::web_gui::config_api::ProxyPick;
use crate::web_gui::http::ConnInfo;
use crate::web_gui::HostError;
use crate::web_gui::{App, SlotHandle};

use super::nets::host_error_response;
use super::slot_param::SlotParam;

/// For an added slot: its config state shows the primary slot's process block.
fn overlay_from_main(app: &App, slot: &SlotHandle) {
    if slot.id.carries_process() {
        return;
    }
    let process = {
        let main = app.host.main_slot();
        let cfg = main.config.lock().unwrap_or_else(|e| e.into_inner());
        cfg.file_settings.process.clone()
    };
    let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.overlay_process(&process);
}

pub(super) async fn config_get_handler(
    State(app): State<Arc<App>>,
    SlotParam(slot): SlotParam,
) -> Response {
    overlay_from_main(&app, &slot);
    let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    (StatusCode::OK, axum::Json(cfg.config_get_body())).into_response()
}

/// The network that `proxy.net` of a main-slot request names, if the body names a well-formed id.
fn named_proxy_net(bytes: &[u8]) -> Option<NetSlotId> {
    let body: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let text = body.get("config")?.get("proxy")?.get("net")?.as_str()?;
    NetSlotId::parse(text)
}

/// A `proxy.net` that names a network this node does not have: 404 before anything is written.
fn refuse_unknown_proxy_net(app: &App, slot: &SlotHandle, bytes: &[u8]) -> Option<Response> {
    if !slot.id.carries_process() {
        return None;
    }
    let id = named_proxy_net(bytes)?;
    match app.host.slot(&id) {
        Some(_) => None,
        None => Some(host_error_response(HostError::UnknownNet)),
    }
}

/// Moves the proxy's exit to the slot a successful write asked for. A failure leaves the answer as it is
/// and goes to the log: the page learns the truth from the network summary.
fn claim_after_write(app: &App, slot: &SlotHandle, pick: ProxyPick) {
    let id = match pick {
        ProxyPick::ThisSlot => slot.id.clone(),
        ProxyPick::Named(id) => id,
    };
    if let Err(e) = app.host.claim_proxy(&id) {
        let text = format!("page: exit chosen in network {id} was not applied: {e:?}");
        crate::node::output::say("web", text);
    }
}

pub(super) async fn config_put_handler(
    State(app): State<Arc<App>>,
    SlotParam(slot): SlotParam,
    conn: Option<Extension<ConnInfo>>,
    bytes: Bytes,
) -> Response {
    if let Some(resp) = refuse_unknown_proxy_net(&app, &slot, &bytes) {
        return resp;
    }
    overlay_from_main(&app, &slot);
    let (resp, pick) = {
        let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
        let (resp, pick) = cfg.put_keeping(&bytes, &*slot.node, conn.map(|Extension(c)| c));
        if resp.status() == StatusCode::OK && slot.id.carries_process() {
            app.shared.set_access(cfg.file_settings.process.web_access);
            if !cfg.cli_override_paths.contains(&"web.ifaces") {
                app.shared
                    .set_ifaces(cfg.file_settings.process.web_ifaces.clone());
            }
        }
        (resp, pick)
    };
    if let Some(pick) = pick {
        claim_after_write(&app, &slot, pick);
    }
    resp
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
