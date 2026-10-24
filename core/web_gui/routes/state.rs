//! `/api/state`: the node snapshot of one slot for the page (the log lives in `log.rs`).

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use super::slot_param::SlotParam;
use super::state_host::{apply_host, apply_notices, apply_slot};
use super::state_net::{apply_net, net_state};
use crate::web_gui::auth::iface_os::current_view;
use crate::web_gui::http::{json_err, ConnInfo};
use crate::web_gui::net_brief::briefs;
use crate::web_gui::{App, InviteJoinView, NetRoles, SlotHandle};

struct StateExtras {
    port: u16,
    config_rev: u64,
    identity: &'static str,
    identity_reason: Option<&'static str>,
    network: Value,
    invite_join: Option<InviteJoinView>,
}

pub(super) async fn state_handler(
    State(app): State<Arc<App>>,
    SlotParam(slot): SlotParam,
    conn: Option<Extension<ConnInfo>>,
) -> Response {
    let conn = conn.map(|Extension(c)| c);
    let Some(mut value) = state_of(&app, &slot, conn.as_ref()) else {
        return json_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal node error",
        );
    };
    // The slot's config lock is released by now; `briefs` takes the locks one by one.
    apply_host(&mut value, &slot.id, briefs(app.host.as_ref()));
    apply_notices(&mut value, app.host.notices());
    (StatusCode::OK, axum::Json(value)).into_response()
}

/// The state of one slot as `/api/state` sends it, without the list of networks. `/api/overview` carries
/// one per slot; the connection-specific keys are those of a request without a connection.
pub(crate) fn slot_state_value(app: &App, slot: &SlotHandle) -> Value {
    state_of(app, slot, None).unwrap_or(Value::Null)
}

fn state_of(app: &App, slot: &SlotHandle, conn: Option<&ConnInfo>) -> Option<Value> {
    let snap = slot.node.snapshot();
    let roles = slot.node.net_roles();
    let mut value = serde_json::to_value(&snap).ok()?;
    let (config_rev, identity, identity_reason, network, restart_pending) = {
        let mut cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
        cfg.sync_boot_baseline(&snap.boot_id);
        let (identity, identity_reason) = cfg.identity_kind_and_reason();
        (
            cfg.file_report.rev,
            identity,
            identity_reason,
            cfg.network_block_with(roles.as_ref()),
            cfg.pending_restart_paths(),
        )
    };
    apply_state_extras(
        &mut value,
        StateExtras {
            port: app.shared.port(),
            config_rev,
            identity,
            identity_reason,
            network,
            invite_join: slot.node.invite_join(),
        },
    );
    let net = net_state(
        &snap,
        &app.shared.auth,
        &current_view(),
        conn,
        restart_pending,
    );
    apply_net(&mut value, net);
    apply_slot(&mut value, &slot.id);
    apply_peer_roles(&mut value, roles.as_ref());
    Some(value)
}

/// `peers[].role` in a managed network: `"admin"` for the listed administrators, `"member"` for the rest.
/// A peer network has no such key.
fn apply_peer_roles(value: &mut Value, roles: Option<&NetRoles>) {
    let Some(roles) = roles.filter(|r| r.managed) else {
        return;
    };
    let Some(peers) = value.get_mut("peers").and_then(Value::as_array_mut) else {
        return;
    };
    for peer in peers.iter_mut().filter_map(Value::as_object_mut) {
        let id = peer.get("id_full").and_then(Value::as_str).unwrap_or("");
        let role = match roles.admins.iter().any(|a| a == id) {
            true => "admin",
            false => "member",
        };
        peer.insert("role".into(), json!(role));
    }
}

/// Inserts derived fields (`config_rev`/`network`/version/address/identity) and rounds the time in
/// `now` and in the log tail to one decimal place, trimming the tail to 6 lines.
fn apply_state_extras(value: &mut Value, extra: StateExtras) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.insert("config_rev".into(), json!(extra.config_rev));
    obj.insert("network".into(), extra.network);
    if let Some(node) = obj.get_mut("node").and_then(Value::as_object_mut) {
        node.insert("version".into(), json!(env!("CARGO_PKG_VERSION")));
        node.insert(
            "web_addr".into(),
            json!(format!("127.0.0.1:{}", extra.port)),
        );
        node.insert("identity".into(), json!(extra.identity));
        node.insert("identity_reason".into(), json!(extra.identity_reason));
        node.insert("invite_join".into(), json!(extra.invite_join));
    }
    if let Some(now) = obj.get_mut("now") {
        *now = json!(round1(now.as_f64().unwrap_or(0.0)));
    }
    if let Some(tail) = obj
        .get_mut("log")
        .and_then(Value::as_object_mut)
        .and_then(|log| log.get_mut("tail"))
        .and_then(Value::as_array_mut)
    {
        for line in tail.iter_mut() {
            if let Some(t) = line.as_object_mut().and_then(|l| l.get_mut("t")) {
                if let Some(x) = t.as_f64() {
                    *t = json!(round1(x));
                }
            }
        }
        let len = tail.len();
        if len > 6 {
            tail.drain(0..len - 6);
        }
    }
}

pub(super) fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "state_extras_tests.rs"]
mod extras_tests;
