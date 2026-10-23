//! The nested menu of several networks as plain data: a submenu per network (header, members, actions) and a group
//! of exits per network. No `muda` here — `nets_rows` turns these into rows and the tray applies the rows.

use super::exit_menu::{exit_menu_from_snapshot, ExitItem};
use super::nets_texts::{net_offline_title_text, net_title_text, JOINING_TITLE, MAX_PEER_ROWS};
use super::overview::SlotView;
use super::snapshot::{TrayPeer, TraySnapshot};
use super::tray_texts::peer_unnamed_text;

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
    /// Pending removal votes and join requests of the network.
    pub voting: u32,
    pub admitting: u32,
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
    slot.label.clone().unwrap_or_else(|| slot.slot.clone())
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
        voting: slot.members_voting,
        admitting: slot.members_admitting,
        label,
    }
}

fn title_of(slot: &SlotView, label: &str) -> String {
    if slot.joining {
        return JOINING_TITLE.to_string();
    }
    if slot.status != "online" {
        return net_offline_title_text(label);
    }
    net_title_text(
        label,
        slot.peers_online,
        slot.peers_total,
        slot.members_admitting,
    )
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
