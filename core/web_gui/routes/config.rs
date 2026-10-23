//! `/api/config`: reading and writing the configuration through `ConfigState`.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::config::config_file::NetSlotId;
use crate::web_gui::http::ConnInfo;
use crate::web_gui::{App, SlotHandle};

use super::slot_param::SlotParam;

/// For an added slot: its config state shows the primary slot's process block.
fn overlay_from_main(app: &App, slot: &SlotHandle) {
    if slot.id == NetSlotId::main() {
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

pub(super) async fn config_put_handler(
    State(app): State<Arc<App>>,
    SlotParam(slot): SlotParam,
    conn: Option<Extension<ConnInfo>>,
    bytes: Bytes,
) -> Response {
    overlay_from_main(&app, &slot);
    let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    let resp = cfg.put_keeping(&bytes, &*slot.node, conn.map(|Extension(c)| c));
    if resp.status() == StatusCode::OK && slot.id == NetSlotId::main() {
        app.shared.set_access(cfg.file_settings.process.web_access);
        if !cfg.cli_override_paths.contains(&"web.ifaces") {
            app.shared
                .set_ifaces(cfg.file_settings.process.web_ifaces.clone());
        }
    }
    resp
}
