//! The network slot a request addresses: `/api/net/{slot}/…` names it, a path without the parameter
//! (`/api/…`) means the primary slot, so one handler serves both addresses.

use std::sync::Arc;

use axum::extract::{FromRequestParts, RawPathParams};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::Response;

use crate::config::config_file::NetSlotId;
use crate::web_gui::http::json_err;
use crate::web_gui::{App, SlotHandle};

/// The slot of the request; an unknown id is a 404 `unknown_net` and the handler is not called.
pub(crate) struct SlotParam(pub SlotHandle);

/// The slot of a request that may also name a broken one (a slot whose files are unusable has no handle).
pub(crate) enum LeaveSlot {
    Live(SlotHandle),
    Broken(NetSlotId),
}

fn unknown_net() -> Response {
    json_err(
        StatusCode::NOT_FOUND,
        "unknown_net",
        "no such network on this computer",
    )
}

/// The `slot` path parameter: `None` when the path has none, `Err` when it is not a slot id.
async fn slot_id(parts: &mut Parts, app: &Arc<App>) -> Result<Option<NetSlotId>, Response> {
    let Ok(params) = RawPathParams::from_request_parts(parts, app).await else {
        return Ok(None);
    };
    let Some((_, text)) = params.iter().find(|(name, _)| *name == "slot") else {
        return Ok(None);
    };
    NetSlotId::parse(text).map(Some).ok_or_else(unknown_net)
}

impl FromRequestParts<Arc<App>> for SlotParam {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Self, Response> {
        match slot_id(parts, app).await? {
            None => Ok(SlotParam(app.host.main_slot())),
            Some(id) => app.host.slot(&id).map(SlotParam).ok_or_else(unknown_net),
        }
    }
}

impl FromRequestParts<Arc<App>> for LeaveSlot {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Self, Response> {
        let Some(id) = slot_id(parts, app).await? else {
            return Ok(LeaveSlot::Live(app.host.main_slot()));
        };
        if let Some(slot) = app.host.slot(&id) {
            return Ok(LeaveSlot::Live(slot));
        }
        match app.host.broken().contains(&id) {
            true => Ok(LeaveSlot::Broken(id)),
            false => Err(unknown_net()),
        }
    }
}
