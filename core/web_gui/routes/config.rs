//! `/api/config`: reading and writing the configuration through `ConfigState`.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::web_gui::http::ConnInfo;
use crate::web_gui::App;

pub(super) async fn config_get_handler(State(app): State<Arc<App>>) -> Response {
    let slot = app.host.main_slot();
    let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    (StatusCode::OK, axum::Json(cfg.config_get_body())).into_response()
}

pub(super) async fn config_put_handler(
    State(app): State<Arc<App>>,
    conn: Option<Extension<ConnInfo>>,
    bytes: Bytes,
) -> Response {
    let slot = app.host.main_slot();
    let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    let resp = cfg.put_keeping(&bytes, &*slot.node, conn.map(|Extension(c)| c));
    if resp.status() == StatusCode::OK {
        app.shared.set_access(cfg.file_settings.process.web_access);
        if !cfg.cli_override_paths.contains(&"web.ifaces") {
            app.shared
                .set_ifaces(cfg.file_settings.process.web_ifaces.clone());
        }
    }
    resp
}
