//! "Leave network" route: the node is told first, then the host retires the slot; a broken slot is only
//! retired.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Map, Value};

use crate::config::config_file::NetSlotId;
use crate::web_gui::http::json_err;
use crate::web_gui::net_brief::briefs;
use crate::web_gui::routes::nets::host_error_response;
use crate::web_gui::routes::slot_param::LeaveSlot;
use crate::web_gui::App;
use crate::web_gui::{MemberActionReq, MembersReply, SlotHandle};

use super::routes::{object_body, reply};

fn confirmed(map: &Map<String, Value>) -> bool {
    matches!(map.get("confirm"), Some(Value::Bool(true)))
}

const KEY_FROM_FLAG: &str =
    "the network key comes from --key or MESHTOWN_KEY: restart without it to leave";

/// What the leave handler needs to know of a live slot before it acts.
struct Standing {
    has_network: bool,
    key_from_flag: bool,
}

fn standing_of(slot: &SlotHandle) -> Standing {
    let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    Standing {
        has_network: cfg.current_network_key.is_some(),
        key_from_flag: cfg.cli_override_paths.contains(&"network.key"),
    }
}

/// Whether some other member is online now, i.e. someone can hear that this node leaves.
fn neighbour_online(slot: &SlotHandle) -> bool {
    let snap = slot.node.snapshot();
    snap.peers.iter().any(|p| !p.is_self && p.online)
}

/// The member list of a node that has no network: no members, nobody excluded or pending.
fn empty_list() -> Map<String, Value> {
    let counts = json!({"members": 0, "voting": 0, "excluded": 0, "unlisted": 0, "pending": 0});
    let Value::Object(map) = json!({
        "me": "",
        "me_excluded": false,
        "me_pending": false,
        "need": 2,
        "me_admission": null,
        "counts": counts,
        "members": [],
    }) else {
        unreachable!("an object literal")
    };
    map
}

/// Takes the slot out of the host and answers with the list of the networks left.
/// The body is the member list of the node (empty when there was no answer) with the leave fields added.
fn retired(
    app: &App,
    id: &NetSlotId,
    left: Option<MembersReply>,
    announced: bool,
) -> Response {
    if let Err(e) = app.host.retire_slot(id, announced) {
        return host_error_response(e);
    }
    let nets = briefs(app.host.as_ref());
    let flag = left.as_ref().and_then(|r| r.changed).unwrap_or(false);
    let mut body = match left.map(serde_json::to_value) {
        Some(Ok(Value::Object(map))) => map,
        _ => empty_list(),
    };
    body.insert("left".into(), json!(flag));
    body.insert("announced".into(), json!(announced));
    body.insert("nets".into(), json!(nets));
    let body = Value::Object(body);
    (StatusCode::OK, axum::Json(body)).into_response()
}

/// Tells the network about leaving (when there is one), then retires the slot. A refusal of the node
/// leaves the slot untouched.
async fn leave_live(app: &App, slot: SlotHandle) -> Response {
    let standing = standing_of(&slot);
    if standing.key_from_flag {
        return json_err(StatusCode::CONFLICT, "busy", KEY_FROM_FLAG);
    }
    if !standing.has_network {
        return retired(app, &slot.id, None, false);
    }
    let announced = neighbour_online(&slot);
    match slot.node.member_action(MemberActionReq::Leave).await {
        Ok(reply) => retired(app, &slot.id, Some(reply), announced),
        Err(e) => reply(Err(e)),
    }
}

pub(super) async fn leave_handler(State(app): State<Arc<App>>, target: LeaveSlot, bytes: Bytes) -> Response {
    let map = match object_body(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    if !confirmed(&map) {
        return json_err(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "field confirm must be true",
        );
    }
    match target {
        LeaveSlot::Live(slot) => leave_live(&app, slot).await,
        LeaveSlot::Broken(id) => retired(&app, &id, None, false),
    }
}

#[cfg(test)]
#[path = "routes_leave_tests.rs"]
mod leave_tests;
