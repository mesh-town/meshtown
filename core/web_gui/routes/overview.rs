//! `/api/overview`: the networks of the process with the state and the member list of each, in one reply.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use super::state::slot_state_value;
use super::state_host::API;
use crate::config::config_file::NetSlotId;
use crate::web_gui::members::members_value;
use crate::web_gui::net_brief::briefs;
use crate::web_gui::{App, SlotHandle};

pub(super) async fn overview_handler(State(app): State<Arc<App>>) -> Response {
    let nets = briefs(app.host.as_ref());
    let mut slots = Vec::new();
    for slot in app.host.slots() {
        slots.push(slot_entry(&app, &slot).await);
    }
    for id in app.host.broken() {
        slots.push(broken_entry(&id));
    }
    let restart_slots = restart_slots(&slots);
    let body = json!({"api": API, "nets": nets, "slots": slots, "restart_slots": restart_slots});
    (StatusCode::OK, axum::Json(body)).into_response()
}

async fn slot_entry(app: &App, slot: &SlotHandle) -> Value {
    let state = slot_state_value(app, slot);
    let members = members_value(slot).await;
    json!({"slot": slot.id.to_string(), "state": state, "members": members})
}

fn broken_entry(id: &NetSlotId) -> Value {
    json!({"slot": id.to_string(), "state": null, "members": null})
}

/// The slots whose state lists settings waiting for a restart, in the order of `slots`.
fn restart_slots(slots: &[Value]) -> Vec<Value> {
    let pending = |entry: &&Value| {
        entry["state"]["restart_pending"]
            .as_array()
            .is_some_and(|paths| !paths.is_empty())
    };
    slots
        .iter()
        .filter(pending)
        .map(|entry| entry["slot"].clone())
        .collect()
}

#[cfg(test)]
#[path = "overview_tests.rs"]
mod tests;
