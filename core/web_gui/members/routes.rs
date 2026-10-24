//! Handlers of `/api/members*`: the list and the member actions. Id checks, busy/left/self/not-member
//! decisions belong to the node; here only the body shape is checked.

use std::sync::Arc;

use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, MethodRouter};
use axum::Router;
use serde_json::{Map, Value};

use crate::web_gui::http::json_err;
use crate::web_gui::routes::helpers::{action_error, bad_method, body_obj};
use crate::web_gui::routes::slot_param::SlotParam;
use crate::web_gui::App;
use crate::web_gui::{MemberActionReq, MemberError, MembersReply, SlotHandle};

use super::leave::leave_handler;

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

pub(super) fn reply(result: Result<MembersReply, MemberError>) -> Response {
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
pub(super) fn object_body(bytes: &[u8]) -> Result<Map<String, Value>, Response> {
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

#[cfg(test)]
#[path = "routes_tests.rs"]
mod tests;

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

#[cfg(test)]
#[path = "routes_forget_slot_tests.rs"]
mod forget_slot_tests;

#[cfg(test)]
#[path = "routes_forget_support_tests.rs"]
mod forget_support;
