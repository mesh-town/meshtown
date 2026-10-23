//! `NetBrief`: the short form of one network slot that the page and the tray show for every slot.
//! It is built from a slot's node snapshot and its config state, so it lives with the page's DTOs.

use serde::Serialize;

use super::control::InviteJoinState;
use super::{HostControl, NodeStatus, RouteMode, SlotHandle};
use crate::config::config_file::NetSlotId;

/// One slot as the page lists it. Field order is the JSON order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NetBrief {
    pub id: String,
    pub label: Option<String>,
    pub id_hex: Option<String>,
    pub status: String,
    pub exit_enabled: bool,
    pub proxy: bool,
    pub route_state: String,
    pub exits: Vec<String>,
    pub members_voting: u64,
    pub joining: bool,
    pub members_admitting: u64,
    pub me_excluded: bool,
    pub me_pending: bool,
    pub peers_online: u64,
    pub peers_total: u64,
}

fn word<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn text_of(block: &serde_json::Value, key: &str) -> Option<String> {
    block.get(key)?.as_str().map(str::to_string)
}

/// The brief of a live slot; `chosen` is the slot that owns the proxy front's exit. The slot's config lock
/// is released before this returns.
pub(crate) fn brief_of(slot: &SlotHandle, chosen: &NetSlotId) -> NetBrief {
    let snap = slot.node.snapshot();
    let block = slot
        .config
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .network_block();
    let peers = || snap.peers.iter().filter(|p| !p.is_self);
    let joining = slot
        .node
        .invite_join()
        .is_some_and(|v| v.state == InviteJoinState::Waiting);
    NetBrief {
        id: slot.id.to_string(),
        label: text_of(&block, "label"),
        id_hex: text_of(&block, "id_hex"),
        status: word::<NodeStatus>(&snap.node.status),
        exit_enabled: snap.exit.enabled,
        proxy: &slot.id == chosen,
        route_state: word::<RouteMode>(&snap.route.state),
        exits: snap.route.exits.iter().map(|e| e.id_full.clone()).collect(),
        members_voting: snap.members.voting,
        joining,
        members_admitting: snap.members.admitting,
        me_excluded: snap.members.me_excluded,
        me_pending: snap.members.me_pending,
        peers_online: peers().filter(|p| p.online).count() as u64,
        peers_total: peers().count() as u64,
    }
}

/// The brief of a slot whose files could not be used: it has no node, so only its id is known.
pub(crate) fn broken_brief(id: &NetSlotId) -> NetBrief {
    NetBrief {
        id: id.to_string(),
        label: None,
        id_hex: None,
        status: "broken".to_string(),
        exit_enabled: false,
        proxy: false,
        route_state: word::<RouteMode>(&RouteMode::Unavailable),
        exits: Vec::new(),
        members_voting: 0,
        joining: false,
        members_admitting: 0,
        me_excluded: false,
        me_pending: false,
        peers_online: 0,
        peers_total: 0,
    }
}

/// Briefs of every live slot in the host's order, then the broken ones. Slot locks are taken one by one.
pub(crate) fn briefs(host: &dyn HostControl) -> Vec<NetBrief> {
    let chosen = host.proxy_net();
    let live = host
        .slots()
        .iter()
        .map(|slot| brief_of(slot, &chosen))
        .collect::<Vec<_>>();
    let broken = host.broken().into_iter().map(|id| broken_brief(&id));
    live.into_iter().chain(broken).collect()
}

#[cfg(test)]
#[path = "net_brief_tests.rs"]
mod tests;
