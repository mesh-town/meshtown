//! Ready-made and assembled tray views: grey by default and three common ones per phase. Shared between
//! `poller.rs` and `poller_phase.rs`/`poller_stop.rs` — hence `pub(super)`.

use crate::gui::tray::app::exit_menu::{exit_menu_from_snapshot, ExitMenu};
use crate::gui::tray::app::nets_menu::{groups_from_slots, nets_from_slots};
use crate::gui::tray::app::overview::{main_slot, parse_overview, visible_slots, SlotView};
use crate::gui::tray::app::proxy_state::ProxyState;
use crate::gui::tray::app::snapshot::TraySnapshot;
use crate::gui::tray::app::view::{
    self, NetLink, NodeState, PageState, RouteLine, TrayColor, View,
};
use crate::gui::tray::app::view_disk;
use crate::gui::tray::app::view_ring::{self, Ring};

pub(super) fn grey_view(page: PageState, node: NodeState) -> View {
    View {
        color: TrayColor::Grey,
        page,
        node,
        ring: Ring::EMPTY,
        disk: None,
        net: NetLink::NoNetwork,
    }
}

/// A view from a `GET /api/state` snapshot (200): the color from the facts, the page is ready, the node is running.
pub(super) fn snapshot_view(snap: &TraySnapshot) -> View {
    snapshot_view_nets(snap, false)
}

/// Like `snapshot_view`, for a node with several networks: when any network is online the icon is not grey and the
/// link is `Joined`, whatever the snapshot of the shown network says.
pub(super) fn snapshot_view_nets(snap: &TraySnapshot, any_online: bool) -> View {
    let color = view::tray_color(online_facts(snap, any_online));
    let net = match any_online {
        true => NetLink::Joined,
        false => view::net_link(snap),
    };
    View {
        color,
        page: PageState::Ready,
        node: NodeState::Running,
        ring: view_ring::ring_of(snap),
        disk: view_disk::disk_of(snap),
        net,
    }
}

fn online_facts(snap: &TraySnapshot, any_online: bool) -> Option<view::ColorFacts> {
    let facts = view::color_facts(snap)?;
    Some(view::ColorFacts {
        online: facts.online || any_online,
        ..facts
    })
}

/// The parsing boundary: the node reply (200) is parsed once, and from it the "Exit via…" submenu and the
/// icon view. A reply with the `nets` and `slots` arrays is the summary of all networks, any other is the state of a
/// single network.
///
/// A snapshot says nothing about the reason for a disabled proxy: the tray's `--no-socks` flag (`tray_proxy_plan`) is added on top.
pub(super) fn views_from_state_json(
    state_json: &serde_json::Value,
    tray_proxy_plan: &ProxyState,
) -> (ExitMenu, View) {
    match parse_overview(state_json) {
        Some(all) => views_from_overview(&all, tray_proxy_plan),
        None => views_from_state(state_json, tray_proxy_plan),
    }
}

fn views_from_state(
    state_json: &serde_json::Value,
    tray_proxy_plan: &ProxyState,
) -> (ExitMenu, View) {
    let snap = TraySnapshot::from_value(state_json);
    let mut menu = exit_menu_from_snapshot(&snap);
    menu.proxy = menu.proxy.with_flag_reason_from(tray_proxy_plan);
    (menu, snapshot_view(&snap))
}

fn views_from_overview(all: &[SlotView], tray_proxy_plan: &ProxyState) -> (ExitMenu, View) {
    let vis = visible_slots(all.to_vec());
    let Some(main) = main_slot(&vis) else {
        return views_from_state(&fallback_state(all), tray_proxy_plan);
    };
    let any_online = vis.iter().any(|s| s.status == "online");
    let snap = TraySnapshot::from_value(&main.state);
    let mut menu = exit_menu_from_snapshot(&snap);
    menu.slot = Some(main.slot.clone());
    menu.proxy = proxy_of(all, &snap).with_flag_reason_from(tray_proxy_plan);
    if vis.len() >= 2 {
        nest(&mut menu, &vis, main);
    }
    (menu, snapshot_view_nets(&snap, any_online))
}

/// The state shown when no network is visible: the main slot, otherwise the first slot that has a state.
fn fallback_state(all: &[SlotView]) -> serde_json::Value {
    let main = all.iter().find(|s| s.slot == "main" && s.state.is_object());
    let first = all.iter().find(|s| s.state.is_object());
    main.or(first).map(|s| s.state.clone()).unwrap_or_default()
}

/// The proxy line comes from the network that carries the proxy, even a hidden one.
fn proxy_of(all: &[SlotView], shown: &TraySnapshot) -> ProxyState {
    let carrier = all.iter().find(|s| s.proxy && s.state.is_object());
    match carrier {
        Some(s) => ProxyState::of_snapshot(&TraySnapshot::from_value(&s.state).socks),
        None => ProxyState::of_snapshot(&shown.socks),
    }
}

/// Two or more networks: submenus and exit groups instead of the flat list, votes summed over the networks.
fn nest(menu: &mut ExitMenu, vis: &[SlotView], main: &SlotView) {
    menu.slot = None;
    menu.items = Vec::new();
    menu.nets = nets_from_slots(vis);
    menu.groups = groups_from_slots(vis);
    menu.members.voting = vis.iter().map(|s| u64::from(s.members_voting)).sum();
    menu.members.admitting = vis.iter().map(|s| u64::from(s.members_admitting)).sum();
    menu.route = via_in_net(menu.route.clone(), main);
}

/// `via peer-one` becomes `via peer-one (friends)`: with several networks the line names the network of the exit.
fn via_in_net(route: RouteLine, main: &SlotView) -> RouteLine {
    match (route, &main.label) {
        (RouteLine::Via(name), Some(label)) => RouteLine::Via(format!("{name} ({label})")),
        (route, _) => route,
    }
}

pub(super) const ABSENT_VIEW: View = View {
    color: TrayColor::Grey,
    page: PageState::Unavailable,
    node: NodeState::Absent,
    ring: Ring::EMPTY,
    disk: None,
    net: NetLink::NoNetwork,
};
pub(super) const STOPPING_VIEW: View = View {
    color: TrayColor::Grey,
    page: PageState::Unavailable,
    node: NodeState::Stopping,
    ring: Ring::EMPTY,
    disk: None,
    net: NetLink::NoNetwork,
};
pub(super) const RUNNING_UNAVAILABLE_VIEW: View = View {
    color: TrayColor::Grey,
    page: PageState::Unavailable,
    node: NodeState::Running,
    ring: Ring::EMPTY,
    disk: None,
    net: NetLink::NoNetwork,
};

#[cfg(test)]
#[path = "views_tests.rs"]
mod tests;
