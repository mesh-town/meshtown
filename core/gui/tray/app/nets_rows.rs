//! Rows of the nested network menu as plain data: what each submenu and the "Exit via…" list contain. The tray applies
//! the rows to `muda`; nothing here touches the OS menu.

use super::exit_menu::ExitMenu;
use super::nets_menu::{ExitGroup, NetMenu, PeerRow};
use super::nets_texts::{
    more_peers_text, open_members_text, peer_offline_row_text, peer_online_text, serve_text,
    MAX_PEER_ROWS, NET_INVITE_TEXT,
};

/// What a row is in the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Item,
    Check,
    Separator,
    /// An inactive title line.
    Header,
}

/// One row of a submenu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowSpec {
    pub id: String,
    pub text: String,
    pub kind: RowKind,
    pub checked: bool,
    pub enabled: bool,
    /// Drawn in the dimmed style.
    pub secondary: bool,
}

impl RowSpec {
    fn new(id: String, text: String, kind: RowKind) -> Self {
        Self {
            id,
            text,
            kind,
            checked: false,
            enabled: true,
            secondary: false,
        }
    }

    fn separator() -> Self {
        Self::new(String::new(), String::new(), RowKind::Separator)
    }

    fn header(id: String, text: String) -> Self {
        Self {
            enabled: false,
            ..Self::new(id, text, RowKind::Header)
        }
    }
}

/// The leaves of "Exit via…" below the permanent "Directly" row: a flat list, or a titled group per network
/// when two or more networks have exits.
pub fn exit_rows(menu: &ExitMenu) -> Vec<RowSpec> {
    if let Some(hint) = menu.empty_hint() {
        return vec![RowSpec {
            enabled: false,
            ..RowSpec::new("exit:hint".to_string(), hint.to_string(), RowKind::Item)
        }];
    }
    if menu.groups.len() >= 2 {
        return grouped_exit_rows(&menu.groups);
    }
    menu.items.iter().map(|item| exit_leaf(item, "")).collect()
}

fn grouped_exit_rows(groups: &[ExitGroup]) -> Vec<RowSpec> {
    let mut rows: Vec<RowSpec> = Vec::new();
    for group in groups.iter().filter(|g| !g.items.is_empty()) {
        if !rows.is_empty() {
            rows.push(RowSpec::separator());
        }
        rows.push(RowSpec::header(
            format!("net:{}", group.slot),
            group.title.clone(),
        ));
        let prefix = format!("{}:", group.slot);
        rows.extend(group.items.iter().map(|item| exit_leaf(item, &prefix)));
    }
    rows
}

fn exit_leaf(item: &super::exit_menu::ExitItem, slot_prefix: &str) -> RowSpec {
    let id = format!("exit:{slot_prefix}{}", item.id_full);
    RowSpec {
        checked: item.checked,
        secondary: !item.online,
        ..RowSpec::new(id, item.menu_text(), RowKind::Check)
    }
}

/// The content of one network submenu. `note` replaces the text of the "Invite a member" row after a copy.
pub fn net_rows(net: &NetMenu, note: Option<&str>) -> Vec<RowSpec> {
    let slot = &net.slot;
    let mut rows = vec![
        RowSpec::header(format!("net:{slot}"), net.title.clone()),
        RowSpec::separator(),
    ];
    rows.extend(peer_lines(net));
    rows.push(RowSpec::new(
        format!("members:{slot}"),
        open_members_text(&net.label),
        RowKind::Item,
    ));
    rows.push(RowSpec::separator());
    let invite_text = note.unwrap_or(NET_INVITE_TEXT).to_string();
    rows.push(RowSpec::new(
        format!("invite:{slot}"),
        invite_text,
        RowKind::Item,
    ));
    rows.push(RowSpec {
        checked: net.serves_exit,
        ..RowSpec::new(
            format!("serve:{slot}"),
            serve_text(&net.label),
            RowKind::Check,
        )
    });
    if net.joining {
        rows.iter_mut().for_each(|row| row.enabled = false);
    }
    rows
}

fn peer_lines(net: &NetMenu) -> Vec<RowSpec> {
    let id = format!("peer:{}", net.slot);
    let mut rows: Vec<RowSpec> = net
        .peers
        .iter()
        .take(MAX_PEER_ROWS)
        .map(|peer| peer_line(&id, peer))
        .collect();
    let rest = net.peers.len().saturating_sub(MAX_PEER_ROWS) + net.peers_more;
    if rest > 0 {
        rows.push(RowSpec::new(id, more_peers_text(rest), RowKind::Item));
    }
    if !rows.is_empty() {
        rows.push(RowSpec::separator());
    }
    rows
}

fn peer_line(id: &str, peer: &PeerRow) -> RowSpec {
    let country = peer.country.as_deref();
    let text = match peer.online {
        true => peer_online_text(&peer.name, country),
        false => peer_offline_row_text(&peer.name, country),
    };
    RowSpec {
        secondary: !peer.online,
        ..RowSpec::new(id.to_string(), text, RowKind::Item)
    }
}

/// The shape of the nested menu: a submenu is rebuilt only when this key changes, otherwise its texts are updated in place.
pub fn layout_key(menu: &ExitMenu) -> String {
    let parts: Vec<String> = menu
        .nets
        .iter()
        .map(|net| format!("{}:{}", net.slot, net_rows(net, None).len()))
        .collect();
    parts.join(",")
}

#[cfg(test)]
#[path = "nets_rows_tests.rs"]
mod tests;
