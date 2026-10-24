//! `POST /api/network/inspect`: what a key or an invite string is, without touching any network.

use axum::body::Bytes;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::members::network_key::{parse_user_input, NetKind};

use super::helpers::{body_obj, invalid_key_response, map_string_field};

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
fn inspect_invite(text: &str) -> Response {
    match crate::invite::parse(text) {
        Ok(p) => answer(
            p.head.label,
            &p.head.kind,
            data_encoding::HEXLOWER.encode(&p.head.network_id),
        ),
        Err(e) => invalid_key_response(&e, &format!("{e:#}")),
    }
}

fn inspect_key(text: &str) -> Response {
    match parse_user_input(text) {
        Ok(i) => answer(i.label, &i.kind, i.key.subkeys().network_id_hex()),
        Err(e) => invalid_key_response(&e, NOT_AN_INVITE),
    }
}

pub(super) async fn network_inspect_handler(bytes: Bytes) -> Response {
    let map = match body_obj(&bytes) {
        Ok(m) => m,
        Err(resp) => return resp,
    };
    let text = match map_string_field(&map, "key") {
        Ok(t) => t,
        Err(resp) => return resp,
    };
    match crate::invite::looks_like_invite(&text) {
        true => inspect_invite(&text),
        false => inspect_key(&text),
    }
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
