//! `POST /api/network/join` without a slot in the address: an invite picks the slot itself.
//! A network this node was removed from or excluded from goes back to its slot, a network that is
//! already here is named, a new one takes the primary slot when it has no network, or a new slot.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::response::Response;

use crate::config::config_file::NetSlotId;
use crate::web_gui::net_brief::{briefs, NetBrief};
use crate::web_gui::{App, HostControl, NodeStatus, SlotHandle};

use super::nets::host_error_response;
use super::network::{join_by_key_response, join_request};
use super::network_invite::{join_by_invite_response, net_already_added};
use super::pending_invites::{forget, remember, waiting_slot};

/// Where an invite to a network goes.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Place {
    /// The node works with this network in `slot`; the invite is not needed.
    Here {
        slot: NetSlotId,
        label: Option<String>,
    },
    /// The node was removed from or excluded from the network in `slot`; the invite brings it back.
    Return(NetSlotId),
    /// A new network and the primary slot has none.
    Main,
    /// A new network and the primary slot is busy: it needs a slot of its own.
    Fresh,
    /// A slot is already waiting for an invite to this network; the repeat replaces that wait.
    Waiting(NetSlotId),
}

impl Place {
    /// The word `inspect` reports for the place.
    pub(super) fn word(&self) -> &'static str {
        match self {
            Place::Here { .. } => "here",
            Place::Return(_) => "return",
            Place::Main | Place::Fresh | Place::Waiting(_) => "new",
        }
    }

    /// The slot the place names, if it names one.
    pub(super) fn slot(&self) -> Option<&NetSlotId> {
        match self {
            Place::Here { slot, .. } | Place::Return(slot) | Place::Waiting(slot) => Some(slot),
            Place::Main | Place::Fresh => None,
        }
    }
}

/// Where the invite to the network `id_hex` goes. A slot whose members cannot be read counts as "here"
/// unless it is excluded or has no running network: an invite is not spent on a network the node works with.
pub(super) async fn place_of(host: &dyn HostControl, id_hex: &str) -> Place {
    let found = briefs(host)
        .into_iter()
        .find(|b| b.id_hex.as_deref() == Some(id_hex) && has_key(host, &b.id));
    match found {
        Some(brief) => place_of_known(host, brief).await,
        None => place_of_new(host),
    }
}

async fn place_of_known(host: &dyn HostControl, brief: NetBrief) -> Place {
    let slot = NetSlotId::parse(&brief.id).unwrap_or_else(NetSlotId::main);
    if brief.me_excluded || needs_return(host, &slot).await {
        return Place::Return(slot);
    }
    Place::Here {
        slot,
        label: brief.label,
    }
}

/// The node was removed from the list, or answers nothing while offline: the slot kept its key but the node has no network (it just left).
async fn needs_return(host: &dyn HostControl, slot: &NetSlotId) -> bool {
    let Some(handle) = host.slot(slot) else {
        return false;
    };
    match handle.node.members().await {
        Ok(_) => false,
        Err(_) => handle.node.snapshot().node.status == NodeStatus::Offline,
    }
}

/// Whether the slot `id` works with a network key; a slot that left its network keeps the id for a while
/// but has no key.
fn has_key(host: &dyn HostControl, id: &str) -> bool {
    let Some(slot) = NetSlotId::parse(id).and_then(|id| host.slot(&id)) else {
        return false;
    };
    let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    cfg.current_network_key.is_some()
}

fn place_of_new(host: &dyn HostControl) -> Place {
    match has_key(host, &NetSlotId::main().to_string()) {
        true => Place::Fresh,
        false => Place::Main,
    }
}

/// Adds a slot, joins it, and gives it back when the node refuses at once (not when it was handed back empty).
fn join_in_fresh_slot(app: &Arc<App>, id_hex: &str, text: &str, keep_ports: bool) -> Response {
    let added = match app.host.add_slot() {
        Ok(a) => a,
        Err(e) => return host_error_response(e),
    };
    let Some(slot) = app.host.slot(&added.id) else {
        return host_error_response(crate::web_gui::HostError::UnknownNet);
    };
    let resp = join_by_invite_response(app, &slot, text, keep_ports);
    if added.fresh && !resp.status().is_success() {
        give_back(app, &slot);
    }
    note_if_accepted(app, id_hex, &slot.id, resp)
}

fn give_back(app: &Arc<App>, slot: &SlotHandle) {
    if let Err(e) = app.host.retire_slot(&slot.id, false) {
        tracing::warn!("could not give back slot {}: {e:?}", slot.id);
    }
}

fn join_in_slot(app: &Arc<App>, id: &NetSlotId, text: &str, keep_ports: bool) -> Response {
    match app.host.slot(id) {
        Some(slot) => join_by_invite_response(app, &slot, text, keep_ports),
        None => host_error_response(crate::web_gui::HostError::UnknownNet),
    }
}

/// Joins `id` with the invite and, when the node took it, notes the slot as waiting for that network.
fn join_and_note(
    app: &Arc<App>,
    id_hex: &str,
    id: &NetSlotId,
    text: &str,
    keep_ports: bool,
) -> Response {
    let resp = join_in_slot(app, id, text, keep_ports);
    note_if_accepted(app, id_hex, id, resp)
}

fn note_if_accepted(app: &Arc<App>, id_hex: &str, id: &NetSlotId, resp: Response) -> Response {
    if resp.status().is_success() {
        remember(app, id_hex, id.clone());
    }
    resp
}

/// The place for an invite: the slot that already waits for it, else what `place_of` picks. A note that
/// no longer holds is dropped by `waiting_slot`.
async fn choose_place(app: &Arc<App>, id_hex: &str) -> Place {
    match waiting_slot(app, id_hex) {
        Some(slot) => Place::Waiting(slot),
        None => place_of(app.host.as_ref(), id_hex).await,
    }
}

/// The invite goes to the slot `place_of` picks; the answer names the slot.
async fn join_by_invite_auto(app: &Arc<App>, text: &str, keep_ports: bool) -> Response {
    let parsed = match crate::invite::parse(text) {
        Ok(p) => p,
        Err(e) => return super::helpers::invalid_key_response(&e, &format!("{e:#}")),
    };
    if let Err(e) = crate::invite::check_fresh(&parsed.head, crate::web_gui::now_unix()) {
        return super::helpers::invalid_key_response(&e, &format!("{e:#}"));
    }
    let id_hex = data_encoding::HEXLOWER.encode(&parsed.head.network_id);
    match choose_place(app, &id_hex).await {
        Place::Here { slot, label } => {
            forget(app, &id_hex);
            net_already_added(&slot, label)
        }
        Place::Return(slot) | Place::Waiting(slot) => {
            join_and_note(app, &id_hex, &slot, text, keep_ports)
        }
        Place::Main => join_and_note(app, &id_hex, &NetSlotId::main(), text, keep_ports),
        Place::Fresh => join_in_fresh_slot(app, &id_hex, text, keep_ports),
    }
}

pub(super) async fn network_join_auto_handler(
    State(app): State<Arc<App>>,
    bytes: Bytes,
) -> Response {
    let (text, keep_ports) = match join_request(&bytes) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    if crate::invite::looks_like_invite(&text) {
        return join_by_invite_auto(&app, &text, keep_ports).await;
    }
    join_by_key_response(&app, &app.host.main_slot(), &text, keep_ports)
}

#[cfg(test)]
#[path = "join_target_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "join_target_route_tests.rs"]
mod route_tests;

#[cfg(test)]
#[path = "join_target_pending_tests.rs"]
mod pending_tests;
