//! The order of the menu items that come and go with the node's state, as plain data. `vote_item`, `extras_ui` and
//! `nets_ui` put their widgets at the places this file names, so the order is decided here and tested without `muda`.
//!
//! Top of the menu: the status line, "Open Mesh Town…", one note per deleted network, the vote item.
//! Middle of the menu: the network submenus (or, for one network, the invite item and the checkbox), then "Add a network…".

use super::exit_menu::ExitMenu;
use crate::gui::tray::core::{MENU_INVITE, MENU_OPEN, MENU_SERVE, MENU_STATUS};
use crate::gui::tray::extras_ui::MENU_ADD;
use crate::gui::tray::vote_item::MENU_VOTE;

/// Prefix of the id of a network submenu; the slot follows.
pub(crate) const NET_MENU_PREFIX: &str = "net-menu:";
/// Prefix of the id of a deleted-network note; the note number follows.
pub(crate) const NOTICE_PREFIX: &str = "notice:";

/// The ids at the top of the menu: status, open, a note per number in `notice_ids`, then the vote item when shown.
pub(crate) fn top_ids(notice_ids: &[u64], vote: bool) -> Vec<String> {
    let mut ids = vec![MENU_STATUS.to_string(), MENU_OPEN.to_string()];
    ids.extend(notice_ids.iter().map(|id| notice_id(*id)));
    if vote {
        ids.push(MENU_VOTE.to_string());
    }
    ids
}

/// The ids of the middle block: submenus of several networks, or the flat invite and checkbox; "Add a network…" last
/// when `has_network`.
pub(crate) fn middle_ids(menu: &ExitMenu, has_network: bool) -> Vec<String> {
    let mut ids: Vec<String> = match menu.nets.len() >= 2 {
        true => menu.nets.iter().map(|n| net_menu_id(&n.slot)).collect(),
        false => vec![MENU_INVITE.to_string(), MENU_SERVE.to_string()],
    };
    if has_network {
        ids.push(MENU_ADD.to_string());
    }
    ids
}

/// Whether the menu shows at least one network: "Add a network…" belongs only to such a menu.
pub(crate) fn has_network(menu: &ExitMenu) -> bool {
    menu.enabled && (menu.slot.is_some() || !menu.nets.is_empty())
}

/// Where the vote item goes in the live menu: right after the notes.
pub(crate) fn vote_position(notice_ids: &[u64]) -> usize {
    top_ids(notice_ids, false).len()
}

pub(crate) fn notice_id(id: u64) -> String {
    format!("{NOTICE_PREFIX}{id}")
}

pub(crate) fn net_menu_id(slot: &str) -> String {
    format!("{NET_MENU_PREFIX}{slot}")
}

#[cfg(test)]
#[path = "menu_order_tests.rs"]
mod tests;
