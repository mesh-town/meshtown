//! Ready-made and assembled tray views: grey by default and three common ones per phase. Shared between
//! `poller.rs` and `poller_phase.rs`/`poller_stop.rs` — hence `pub(super)`.

use crate::app::exit_menu::{exit_menu_from_snapshot, ExitMenu};
use crate::app::proxy_state::ProxyState;
use crate::app::snapshot::TraySnapshot;
use crate::app::view::{self, NetLink, NodeState, PageState, TrayColor, View};
use crate::app::view_disk;
use crate::app::view_ring::{self, Ring};

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
    let color = view::tray_color(view::color_facts(snap));
    View {
        color,
        page: PageState::Ready,
        node: NodeState::Running,
        ring: view_ring::ring_of(snap),
        disk: view_disk::disk_of(snap),
        net: view::net_link(snap),
    }
}

/// The parsing boundary: the node reply (200) is parsed into `TraySnapshot` once, and from it the
/// "Exit via…" submenu and the icon view.
///
/// A snapshot says nothing about the reason for a disabled proxy: the tray's `--no-socks` flag (`tray_proxy_plan`) is added on top.
pub(super) fn views_from_state_json(
    state_json: &serde_json::Value,
    tray_proxy_plan: &ProxyState,
) -> (ExitMenu, View) {
    let snap = TraySnapshot::from_value(state_json);
    let mut menu = exit_menu_from_snapshot(&snap);
    menu.proxy = menu.proxy.with_flag_reason_from(tray_proxy_plan);
    (menu, snapshot_view(&snap))
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
