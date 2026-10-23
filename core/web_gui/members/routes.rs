//! Handlers of `/api/members*`: the list and the member actions. Id checks, busy/left/self/not-member
//! decisions belong to the node; here only the body shape is checked.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, MethodRouter};
use axum::Router;
use serde_json::{json, Map, Value};

use crate::config::config_file::NetSlotId;
use crate::web_gui::http::json_err;
use crate::web_gui::net_brief::briefs;
use crate::web_gui::routes::helpers::{action_error, bad_method, body_obj};
use crate::web_gui::routes::nets::host_error_response;
use crate::web_gui::routes::slot_param::{LeaveSlot, SlotParam};
use crate::web_gui::App;
use crate::web_gui::{MemberActionReq, MemberError, MembersReply, SlotHandle};

/// Registers the member routes of one network under `base` (`/api` or `/api/net/{slot}`).
pub(crate) fn register_members(router: Router<Arc<App>>, base: &str) -> Router<Arc<App>> {
    let actions: [(&str, MethodRouter<Arc<App>>); 9] = [
        ("exclude", post(exclude_handler)),
        ("unexclude", post(unexclude_handler)),
        ("admit", post(admit_handler)),
        ("unadmit", post(unadmit_handler)),
        ("block", post(block_handler)),
        ("unblock", post(unblock_handler)),
        ("forget", post(forget_handler)),
        ("unforget", post(unforget_handler)),
        ("leave", post(leave_handler)),
    ];
    let router = router.route(
        &format!("{base}/members"),
        get(members_handler).fallback(bad_method),
    );
    actions.into_iter().fold(router, |router, (name, route)| {
        router.route(
            &format!("{base}/members/{name}"),
            route.fallback(bad_method),
        )
    })
}

fn reply(result: Result<MembersReply, MemberError>) -> Response {
    match result {
        Ok(body) => (StatusCode::OK, axum::Json(body)).into_response(),
        Err(MemberError::Action(e)) => action_error(e),
        Err(MemberError::Refused { code, message }) => {
            json_err(StatusCode::CONFLICT, code, &message)
        }
    }
}

async fn members_handler(SlotParam(slot): SlotParam) -> Response {
    reply(slot.node.members().await)
}

/// The member list of a slot as the list route would send it; `None` where the route answers an error
/// (no network, a refusal).
pub(crate) async fn members_value(slot: &SlotHandle) -> Option<Value> {
    let reply = slot.node.members().await.ok()?;
    serde_json::to_value(reply).ok()
}

/// The body as a JSON object; anything else (including non-JSON) is 400 `bad_request`.
fn object_body(bytes: &[u8]) -> Result<Map<String, Value>, Response> {
    body_obj(bytes).map_err(|_| {
        json_err(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "body must be a JSON object",
        )
    })
}

/// Runs an action that takes the target `id` from the body, built by `make`.
async fn id_action(
    slot: &SlotHandle,
    bytes: &Bytes,
    make: fn(String) -> MemberActionReq,
) -> Response {
    let map = match object_body(bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    match map.get("id") {
        Some(Value::String(id)) if !id.trim().is_empty() => {
            reply(slot.node.member_action(make(id.trim().to_string())).await)
        }
        _ => json_err(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "field id is required",
        ),
    }
}

/// Longest reason of an exclusion vote, in characters.
const MAX_REASON_CHARS: usize = 80;

/// The optional `reason`: absent or `null` is none; a blank string is none; a non-string, a long one or
/// one with a control character (a line break included, also at the ends) is a `bad_request`.
fn reason_of(map: &Map<String, Value>) -> Result<Option<String>, Response> {
    let text = match map.get("reason") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(s)) => s,
        Some(_) => return Err(bad_reason("field reason must be a string")),
    };
    if text.chars().any(char::is_control) {
        return Err(bad_reason(
            "field reason must not contain control characters",
        ));
    }
    let text = text.trim();
    if text.chars().count() > MAX_REASON_CHARS {
        return Err(bad_reason("field reason must be at most 80 characters"));
    }
    Ok(Some(text.to_string()).filter(|s| !s.is_empty()))
}

fn bad_reason(message: &str) -> Response {
    json_err(StatusCode::BAD_REQUEST, "bad_request", message)
}

async fn exclude_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    let map = match object_body(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let id = match map.get("id") {
        Some(Value::String(id)) if !id.trim().is_empty() => id.trim().to_string(),
        _ => {
            return json_err(
                StatusCode::BAD_REQUEST,
                "bad_request",
                "field id is required",
            )
        }
    };
    match reason_of(&map) {
        Ok(reason) => reply(
            slot.node
                .member_action(MemberActionReq::Exclude { id, reason })
                .await,
        ),
        Err(resp) => resp,
    }
}

async fn unexclude_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    id_action(&slot, &bytes, MemberActionReq::Unexclude).await
}

async fn admit_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    id_action(&slot, &bytes, MemberActionReq::Admit).await
}

async fn unadmit_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    id_action(&slot, &bytes, MemberActionReq::Unadmit).await
}

async fn block_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    id_action(&slot, &bytes, MemberActionReq::Block).await
}

async fn unblock_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    id_action(&slot, &bytes, MemberActionReq::Unblock).await
}

async fn forget_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    id_action(&slot, &bytes, MemberActionReq::Forget).await
}

async fn unforget_handler(SlotParam(slot): SlotParam, bytes: Bytes) -> Response {
    id_action(&slot, &bytes, MemberActionReq::Unforget).await
}

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
    if slot.id == NetSlotId::main() && standing.key_from_flag {
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

async fn leave_handler(State(app): State<Arc<App>>, target: LeaveSlot, bytes: Bytes) -> Response {
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
#[path = "routes_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "routes_leave_tests.rs"]
mod leave_tests;

#[cfg(test)]
#[path = "routes_actions_tests.rs"]
mod action_tests;

#[cfg(test)]
#[path = "routes_fixture_tests.rs"]
mod fixture;

#[cfg(test)]
#[path = "routes_admit_tests.rs"]
mod admit_tests;

#[cfg(test)]
#[path = "routes_forget_tests.rs"]
mod forget_tests;
