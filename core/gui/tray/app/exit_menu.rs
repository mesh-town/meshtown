//! The tray submenu "Exit via…": a pure fold of a node snapshot selection (`TraySnapshot`) into a list of items —
//! other nodes advertising themselves as an exit (`peers[].exit`) and already chosen ones (`route.exits[]`),
//! even if offline right now. The own node is excluded from both sides of the union. The same snapshot
//! builds `route: RouteLine` (for `node_status_menu_line`) and `proxy: ProxyState` — the address of the
//! browser SOCKS5 from `socks.listen`, if `socks.enabled`, otherwise
//! "off" (the tray sets the reason on top of the snapshot, `proxy_state`).
//! `ExitItem::online` and `menu_text()` add grey color to an offline item and a red sign
//! to a chosen offline exit — `tray::exit_menu` does the coloring and applies the sign.
//! What it doesn't do: it knows nothing about HTTP or `tray-icon`/`muda` — that is `tray::exit_menu`.
//! Links: builds `ExitMenu`/`ExitItem`; `tray_role::poller` calls `exit_menu_from_snapshot` (through `views_from_state_json`) on every
//! snapshot and `disabled_exit_menu` on grey views (the node is not running — the submenu is unavailable).

use crate::web_gui::RouteMode;

use crate::gui::tray::app::nets_menu::{ExitGroup, NetMenu};
use crate::gui::tray::app::proxy_state::{ProxyOff, ProxyState};
use crate::gui::tray::app::snapshot::{
    TrayChosenExit, TrayMembers, TrayPeer, TrayRoute, TraySnapshot,
};
use crate::gui::tray::app::tray_texts::{peer_offline_text, peer_unnamed_text, EMPTY_HINT};
use crate::gui::tray::app::view::RouteLine;

/// A ready "Exit via…" submenu list: availability, items, the status line (`route`)
/// and the browser proxy address (`proxy`) for the `MENU_PROXY` item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitMenu {
    pub enabled: bool,
    pub items: Vec<ExitItem>,
    pub route: RouteLine,
    pub proxy: ProxyState,
    /// The checkbox "Allow opening sites via this computer": `exit.enabled` of the snapshot.
    pub serves_exit: bool,
    /// `members` of the snapshot: the removal vote and our own exclusion (`members_note`).
    pub members: TrayMembers,
    /// The network the flat menu shows; `None` — the main network (or no network at all).
    pub slot: Option<String>,
    /// A submenu per network; filled only when two or more networks are shown.
    pub nets: Vec<NetMenu>,
    /// The exits per network, for "Exit via…"; filled together with `nets`, then `items` is empty.
    pub groups: Vec<ExitGroup>,
}

/// One submenu item: a node, an identifier for tray commands, a ready label, the "chosen" mark and
/// "online" (the grey color and the red sign of an offline exit are applied by `tray::exit_menu`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitItem {
    pub id_full: String,
    pub label: String,
    pub checked: bool,
    pub online: bool,
}

/// The sign next to a chosen exit that is offline.
pub const OFFLINE_EXIT_MARK: &str = "🔴";

impl ExitMenu {
    /// An inactive hint instead of the member list when there are no exits.
    pub fn empty_hint(&self) -> Option<&'static str> {
        let grouped = self.groups.iter().any(|g| !g.items.is_empty());
        (self.items.is_empty() && !grouped).then_some(EMPTY_HINT)
    }
}

impl ExitItem {
    /// The menu label: a chosen offline exit gets a red sign and a space in front,
    /// other items are shown as `label` has them.
    pub fn menu_text(&self) -> String {
        if self.checked && !self.online {
            format!("{OFFLINE_EXIT_MARK} {}", self.label)
        } else {
            self.label.clone()
        }
    }
}

/// The submenu is unavailable (the node is not running). Used on grey views. `proxy` is the proxy
/// address from the settings file (`None` — the proxy is off there): the proxy line in the menu doesn't depend on
/// whether the node is alive.
pub fn disabled_exit_menu(proxy: ProxyState) -> ExitMenu {
    ExitMenu {
        enabled: false,
        items: Vec::new(),
        route: RouteLine::Direct,
        proxy,
        serves_exit: false,
        members: TrayMembers::default(),
        slot: None,
        nets: Vec::new(),
        groups: Vec::new(),
    }
}

/// Snapshot → submenu. The sanity gate is the same as in `color_facts`: no `node` object
/// means this is not a node snapshot at all (serialization failed or the node hasn't answered yet) — unavailable.
/// A valid snapshot without `route` (an old node) is available, but the list is empty: there is neither a choice nor
/// exit advertising without this field.
pub fn exit_menu_from_snapshot(snap: &TraySnapshot) -> ExitMenu {
    let Some(node) = snap.node.as_ref() else {
        return disabled_exit_menu(ProxyState::Off(ProxyOff::DisabledInSettings));
    };
    let own_id = node.id_full.as_str();

    let mut items: Vec<ExitItem> = Vec::new();
    for exit in snap.route.exits.iter().filter_map(chosen_exit_item) {
        push_unless_own_or_duplicate(&mut items, exit, own_id);
    }
    for item in advertising_exit_items(snap) {
        push_unless_own_or_duplicate(&mut items, item, own_id);
    }

    ExitMenu {
        enabled: !snap.members.me_excluded,
        items,
        route: route_line(&snap.route),
        proxy: ProxyState::of_snapshot(&snap.socks),
        serves_exit: snap.exit.enabled,
        members: snap.members,
        slot: None,
        nets: Vec::new(),
        groups: Vec::new(),
    }
}

/// `route.state`/`route.exits[]` of the snapshot → `RouteLine`. No `route` at all (an old snapshot) —
/// like an empty choice: `Direct`.
fn route_line(route: &TrayRoute) -> RouteLine {
    let exits = route.exits.as_slice();
    match route.state {
        RouteMode::Exit => exits
            .iter()
            .find(|e| e.active)
            .map(|e| RouteLine::Via(exit_route_label(e)))
            .unwrap_or(RouteLine::Direct),
        RouteMode::Unavailable if exits.len() == 1 => {
            RouteLine::Unavailable(Some(exit_route_label(&exits[0])))
        }
        RouteMode::Unavailable => RouteLine::Unavailable(None),
        RouteMode::Direct => RouteLine::Direct,
    }
}

/// The label of the chosen exit in the first menu line — the same as the submenu item's, but without
/// the "offline" mark (the line writes it itself): "name · CODE" or "member {id}".
fn exit_route_label(exit: &TrayChosenExit) -> String {
    let id = exit.id.as_deref().unwrap_or("");
    peer_exit_caption(exit.name.as_deref(), id, true, exit.country.as_deref())
}

/// Other nodes advertising themselves as an exit (`peers[].exit`) are not marked here; the "chosen"
/// mark appears through dedup with `route.exits` in `push_unless_own_or_duplicate`.
fn advertising_exit_items(snap: &TraySnapshot) -> Vec<ExitItem> {
    let peers = snap.peers.as_deref().unwrap_or_default();
    peers
        .iter()
        .filter(|p| advertises_exit(p))
        .filter_map(peer_item)
        .collect()
}

fn advertises_exit(peer: &TrayPeer) -> bool {
    !peer.is_self && peer.exit
}

/// An already chosen exit (`route.exits[]`; an old config may hold several) — in
/// config order, even offline; without `id_full` there is no item.
fn chosen_exit_item(exit: &TrayChosenExit) -> Option<ExitItem> {
    let id_full = exit.id_full.clone()?;
    let id = exit.id.as_deref().unwrap_or(&id_full);
    let caption = peer_exit_caption(
        exit.name.as_deref(),
        id,
        exit.online,
        exit.country.as_deref(),
    );
    Some(ExitItem {
        id_full,
        label: caption,
        checked: true,
        online: exit.online,
    })
}

fn peer_item(peer: &TrayPeer) -> Option<ExitItem> {
    let id_full = peer.id_full.clone()?;
    let id = peer.id.as_deref().unwrap_or(&id_full);
    let caption = peer_exit_caption(
        peer.name.as_deref(),
        id,
        peer.online,
        peer.country.as_deref(),
    );
    Some(ExitItem {
        id_full,
        label: caption,
        checked: false,
        online: peer.online,
    })
}

/// The node name or "member <short id>" if there is no name, then the country code (an old
/// node without a `country` field in JSON — a label without a country, a non-string value too); offline —
/// a suffix after the country (another node gets a name in the handshake when
/// connecting and may be without one — for example known only from the address cache on disk, where
/// there is no name at all — the label must not stay empty).
fn peer_exit_caption(name: Option<&str>, id: &str, online: bool, country: Option<&str>) -> String {
    let base = match name {
        Some(n) => n.to_string(),
        None => peer_unnamed_text(id),
    };
    let base = match country {
        Some(c) => format!("{base} · {c}"),
        None => base,
    };
    if online {
        base
    } else {
        peer_offline_text(&base)
    }
}

/// Adds an item if it is not the own node and no such `id_full` is in the list yet (the first one is from
/// `chosen_exits`, so the chosen one wins on duplication).
fn push_unless_own_or_duplicate(items: &mut Vec<ExitItem>, item: ExitItem, own_id: &str) {
    if item.id_full == own_id {
        return;
    }
    if items.iter().any(|i| i.id_full == item.id_full) {
        return;
    }
    items.push(item);
}

#[cfg(test)]
#[path = "exit_menu_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "exit_menu_text_tests.rs"]
mod text_tests;

#[cfg(test)]
#[path = "exit_menu_country_tests.rs"]
mod country_tests;

#[cfg(test)]
#[path = "exit_menu_route_tests.rs"]
mod route_tests;
