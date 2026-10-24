//! `POST /api/network/inspect`: what a key or an invite string is, without touching any network.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::members::network_key::{parse_user_input, NetKind};

use crate::web_gui::App;

use super::helpers::{body_obj, invalid_key_response, map_string_field};
use super::join_target::place_of;

const NOT_AN_INVITE: &str = "This doesn't look like an invite. Copy it from the network member again in full and paste it here.";

fn kind_word(kind: &NetKind) -> &'static str {
    match kind.root() {
        Some(_) => "Managed",
        None => "Peer",
    }
}

fn answer(label: Option<String>, kind: &NetKind, id_hex: String) -> Response {
    let body = json!({"label": label, "kind": kind_word(kind), "id_hex": id_hex});
    (StatusCode::OK, axum::Json(body)).into_response()
}

/// An invite string: parsed and checked for the format only; it is not redeemed, so it is not spent.
/// The answer also says where a join would put it (`place`, and `slot` when the place names one).
async fn inspect_invite(app: &App, text: &str) -> Response {
    let p = match crate::invite::parse(text) {
        Ok(p) => p,
        Err(e) => return invalid_key_response(&e, &format!("{e:#}")),
    };
    let id_hex = data_encoding::HEXLOWER.encode(&p.head.network_id);
    let place = place_of(app.host.as_ref(), &id_hex).await;
    let body = json!({
        "label": p.head.label,
        "kind": kind_word(&p.head.kind),
        "id_hex": id_hex,
        "place": place.word(),
        "slot": place.slot().map(|s| s.to_string()),
    });
    (StatusCode::OK, axum::Json(strip_null_slot(body))).into_response()
}

fn strip_null_slot(mut body: serde_json::Value) -> serde_json::Value {
    if body["slot"].is_null() {
        body.as_object_mut().map(|m| m.remove("slot"));
    }
    body
}

fn inspect_key(text: &str) -> Response {
    match parse_user_input(text) {
        Ok(i) => answer(i.label, &i.kind, i.key.subkeys().network_id_hex()),
        Err(e) => invalid_key_response(&e, NOT_AN_INVITE),
    }
}

pub(super) async fn network_inspect_handler(State(app): State<Arc<App>>, bytes: Bytes) -> Response {
    let map = match body_obj(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let text = match map_string_field(&map, "key") {
        Ok(t) => t,
        Err(resp) => return resp,
    };
    match crate::invite::looks_like_invite(&text) {
        true => inspect_invite(&app, &text).await,
        false => inspect_key(&text),
    }
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
