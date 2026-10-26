//! The nested menu of several networks as plain data: a submenu per network (header, members, actions) and a group
//! of exits per network. No `muda` here — `nets_rows` turns these into rows and the tray applies the rows.

use super::exit_menu::{exit_menu_from_snapshot, ExitItem};
use super::nets_texts::{
    admin_tail, net_offline_title_text, net_title_text, JOINING_TITLE, MAX_PEER_ROWS,
};
use super::overview::SlotView;
use super::snapshot::{TrayPeer, TraySnapshot};
use super::tray_texts::peer_unnamed_text;
use crate::web_gui::net_name::net_display_name;

/// One member line of a network submenu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerRow {
    pub name: String,
    pub online: bool,
    pub exit: bool,
    pub country: Option<String>,
}

/// A submenu of one network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetMenu {
    pub slot: String,
    /// The inactive header line.
    pub title: String,
    pub joining: bool,
    pub online: bool,
    pub peers: Vec<PeerRow>,
    /// Members that are not in `peers` but exist (listed in the "and N more" line).
    pub peers_more: usize,
    pub serves_exit: bool,
    /// Join requests of the network that wait for an admin.
    pub admitting: u32,
    /// Administered network where we may not invite: the invite row is grey.
    pub invite_admins_only: bool,
    /// The network name for texts; the slot id when it has no name.
    pub label: String,
}

/// The exits of one network, under its own header in "Exit via…".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitGroup {
    pub slot: String,
    pub title: String,
    pub items: Vec<ExitItem>,
}

pub fn nets_from_slots(vis: &[SlotView]) -> Vec<NetMenu> {
    vis.iter().map(net_menu).collect()
}

pub fn groups_from_slots(vis: &[SlotView]) -> Vec<ExitGroup> {
    vis.iter().filter(|s| !s.joining).map(exit_group).collect()
}

fn label_of(slot: &SlotView) -> String {
    let named = net_display_name(slot.label.as_deref(), slot.id_hex.as_deref());
    named.unwrap_or_else(|| slot.slot.clone())
}

fn net_menu(slot: &SlotView) -> NetMenu {
    let snap = TraySnapshot::from_value(&slot.state);
    let label = label_of(slot);
    let mut peers = peer_rows(&snap);
    let peers_more = peers.len().saturating_sub(MAX_PEER_ROWS);
    peers.truncate(MAX_PEER_ROWS);
    NetMenu {
        slot: slot.slot.clone(),
        title: title_of(slot, &label),
        joining: slot.joining,
        online: slot.status == "online",
        peers,
        peers_more,
        serves_exit: snap.exit.enabled,
        admitting: slot.members_admitting,
        invite_admins_only: !slot.can_invite,
        label,
    }
}

fn title_of(slot: &SlotView, label: &str) -> String {
    if slot.joining {
        return JOINING_TITLE.to_string();
    }
    let base = match slot.status == "online" {
        true => net_title_text(
            label,
            slot.peers_online,
            slot.peers_total,
            slot.members_admitting,
        ),
        false => net_offline_title_text(label),
    };
    format!("{base}{}", admin_tail(slot.admin))
}

fn peer_rows(snap: &TraySnapshot) -> Vec<PeerRow> {
    let peers = snap.peers.as_deref().unwrap_or_default();
    peers.iter().filter(|p| !p.is_self).map(peer_row).collect()
}

fn peer_row(peer: &TrayPeer) -> PeerRow {
    let id = peer.id.as_deref().or(peer.id_full.as_deref()).unwrap_or("");
    PeerRow {
        name: peer.name.clone().unwrap_or_else(|| peer_unnamed_text(id)),
        online: peer.online,
        exit: peer.exit,
        country: peer.country.clone(),
    }
}

fn exit_group(slot: &SlotView) -> ExitGroup {
    let snap = TraySnapshot::from_value(&slot.state);
    ExitGroup {
        slot: slot.slot.clone(),
        title: label_of(slot),
        items: exit_menu_from_snapshot(&snap).items,
    }
}

#[cfg(test)]
#[path = "nets_menu_tests.rs"]
mod tests;
