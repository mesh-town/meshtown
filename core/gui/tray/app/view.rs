//! What the tray icon shows: the color and the menu texts.
//!
//! What it does: turns the node snapshot selection (`TraySnapshot` from `GET /api/state`, `docs/WEB-API.md` §1.3) into plain facts
//! (`ColorFacts`), the facts into a color (`TrayColor`) and draws it as a circle (`icon_rgba`); the page
//! state (`PageState`) into the text and availability of the "Open" menu item; the node state
//! (`NodeState`) into the text and availability of the single node item `node_item_text` (it used to be
//! two items, "Start" and "Stop"); `node_status_menu_line` is the status menu line, built from
//! the view and a `RouteLine` (the selected exit, assembled by `exit_menu.rs`).
//! What it does not do: it does not read the network and knows nothing of the node — its only input is `&TraySnapshot`.
//! Links: `tray` calls `icon_rgba`/`page_text`/`node_status_menu_line`/`node_item_text`/
//! `start_enabled`/`stop_enabled` on receiving `TrayCmd::Update`; `View` is assembled by the poller of
//! `tray_role` from the `GET /api/state` response and its own phase.

use crate::web_gui::RouteMode;

use super::exit_menu::OFFLINE_EXIT_MARK;
use super::snapshot::{TrayNodeStatus, TraySnapshot};
use super::tray_texts::{
    page_open_text, port_busy_hint_text, route_direct_text, route_exit_offline_text,
    route_via_text, status_absent_text, status_starting_text, status_unresponsive_text,
    DISABLED_HINT, NODE_ITEM_STOPPING_TEXT, PAGE_STARTING_TEXT, PAGE_UNAVAILABLE_TEXT,
    ROUTE_ALL_OFFLINE_TEXT, START_TEXT, STATUS_STOPPING_TEXT, STOP_TEXT,
};
use super::view_disk::Disk;
use super::view_ring::Ring;

// The name of our node network in texts for people is `crate::node::output::NET_NAME` (its doc comment
// is there). Not here: almost all places that use it are in the library crate `meshtown`, and
// `app` exists only in the binary crate `meshtown`, so a constant in `app` would not be visible
// from there unless it were `pub`; the binary crate sees it through `crate::node::output::NET_NAME`
// (like `commands.rs`).

pub const ICON_SIDE_PX: u32 = 36;

/// The tray icon color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayColor {
    Grey,
    Green,
    Red,
    /// In a network, sites open as usual (nothing selected): a green ring without a fill.
    GreenOutline,
}

/// All colors in the order of the tray icon array (`tray/menu_build.rs::build_icons`).
pub const TRAY_COLORS: [TrayColor; 4] = [
    TrayColor::Grey,
    TrayColor::Green,
    TrayColor::Red,
    TrayColor::GreenOutline,
];

/// Facts about the node state from which the color is computed. The source is `color_facts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorFacts {
    pub online: bool,
    pub exit_available: bool,
    /// Sites go via a selected member (`route.state == "exit"`).
    pub via_peer: bool,
}

/// The node page state — determines the text and availability of the "Open" menu item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageState {
    Unavailable,
    Starting,
    Ready,
    Disabled,
    PortBusy(u16),
}

/// The node state as the icon sees it — determines the text and availability of the single
/// node item (`node_item_text`, `start_enabled`/`stop_enabled`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeState {
    /// Before the start decision, the port is in use by something other than the node, or the page is turned off.
    Unknown,
    /// There is no node: it was not started, was stopped or disappeared.
    Absent,
    /// Our own node is started, no 200 reply yet.
    Starting,
    /// There was a 200 reply and the node has not disappeared (a temporary page failure is still `Running`).
    Running,
    /// The stop procedure is under way.
    Stopping,
}

/// What the node snapshot says about the node's network (the reason for a grey icon when the page is ready).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetLink {
    /// No network (`network: null`, no `node`): the node has not joined a network.
    NoNetwork,
    /// There is a network and the node is connecting (`starting`, restart `stopping`).
    Connecting,
    /// There is a network and no connection (`offline` and an unknown status).
    Lost,
    /// There is a network and the node is in it (never grey).
    Joined,
}

/// The full icon state: color, page state, node state and the composition of the ring of sectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub color: TrayColor,
    pub page: PageState,
    pub node: NodeState,
    /// Roles of the members for the icon's sectors (`view_ring`); empty means a solid icon, except for our own
    /// node in `Absent`/`Starting`/`Stopping`, where the tray draws a grey ring.
    pub ring: Ring,
    /// The exit traffic speed disk in the centre of the icon (`view_disk`); `None` means no traffic.
    pub disk: Option<Disk>,
    /// What the snapshot says about the node's network; affects only the texts of the grey `Ready`/`Running` view.
    pub net: NetLink,
}

impl View {
    /// The initial state before the first poll of the node: a grey circle, the page still starting, the node
    /// not visible yet.
    pub fn initial() -> Self {
        View {
            color: TrayColor::Grey,
            page: PageState::Starting,
            node: NodeState::Unknown,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        }
    }
}

/// "Start Mesh Town" is available only when there is no node.
pub fn start_enabled(node: NodeState) -> bool {
    matches!(node, NodeState::Absent)
}

/// "Stop Mesh Town" is available while the node is starting or running.
pub fn stop_enabled(node: NodeState) -> bool {
    matches!(node, NodeState::Starting | NodeState::Running)
}

/// The third part of the view-change line.
pub fn node_state_phrase(node: NodeState) -> &'static str {
    match node {
        NodeState::Unknown => "Mesh Town not visible",
        NodeState::Absent => "Mesh Town is not running",
        NodeState::Starting => "Mesh Town is starting",
        NodeState::Running => "Mesh Town is running",
        NodeState::Stopping => "Mesh Town is stopping",
    }
}

/// Extracts the facts from the node snapshot selection (`TraySnapshot`, the shape of `docs/WEB-API.md` §1.3).
///
/// `None` only when there is no `node` or `node.status` is not a string (the single failure). Otherwise:
/// `network` counts **only** when it is `null` — a missing key is not the same as `null`
/// (the `network` field in the `GET /api/state` response is appended by the listener, `routes.rs:183`).
/// `exit_available` comes from `route.state`: `Unavailable` means no, anything else (`Direct`, `Exit`,
/// a missing `route` key altogether — an old snapshot without exit selection) means yes; the list
/// `route.exits` and `peers[].exit` no longer affect the color, that is the submenu's business (`exit_menu.rs`).
/// A type mismatch in a single field means the fact is absent (`false`), not a panic and not a general
/// `None`.
pub fn color_facts(snap: &TraySnapshot) -> Option<ColorFacts> {
    let status = snap.node.as_ref()?.status?;
    let online = status == TrayNodeStatus::Online && !snap.network_is_null;
    let route = snap.route.state;
    Some(ColorFacts {
        online,
        exit_available: route != RouteMode::Unavailable,
        via_peer: route == RouteMode::Exit,
    })
}

/// Facts → color (already folded into `online`): no facts or the node is not online — grey; the selected
/// exits are offline — red; via a member — a green circle; as usual — a green ring.
pub fn tray_color(facts: Option<ColorFacts>) -> TrayColor {
    match facts {
        None => TrayColor::Grey,
        Some(f) if !f.online => TrayColor::Grey,
        Some(f) if !f.exit_available => TrayColor::Red,
        Some(f) if f.via_peer => TrayColor::Green,
        Some(_) => TrayColor::GreenOutline,
    }
}

/// The line about the selected exit — the third field of `ExitMenu` (`exit_menu.rs`),
/// assembled from `route.state`/`route.exits[]` of the node snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteLine {
    /// No exits selected — connections go direct.
    Direct,
    /// Exactly one exit selected and it is online: the name or (with no name) the short `id`.
    Via(String),
    /// The selected exit (or exits) is offline right now. `Some(name)` means exactly one is selected;
    /// `None` means several are selected and none is online.
    Unavailable(Option<String>),
}

/// The status menu item line (`MENU_STATUS` in `tray/menu_build.rs`): the node state,
/// and while the node is running and responding, a short note about the selected exit (`route`, see `exit_menu.rs`).
pub fn node_status_menu_line(view: View, route: &RouteLine) -> String {
    match view.node {
        NodeState::Absent => status_absent_text(),
        NodeState::Starting => status_starting_text(),
        NodeState::Stopping => STATUS_STOPPING_TEXT.to_string(),
        NodeState::Unknown => unknown::unknown_status(view.page),
        NodeState::Running if view.page == PageState::Unavailable => status_unresponsive_text(),
        NodeState::Running if outside_network(view) => net::grey_status(view.net),
        NodeState::Running => route_line_text(route),
    }
}

/// The node is running and the page is ready, but it has not joined a network (grey icon).
fn outside_network(view: View) -> bool {
    view.node == NodeState::Running
        && view.color == TrayColor::Grey
        && view.page == PageState::Ready
}

/// The third part of the status line for `Running`: text by the selected exit.
fn route_line_text(route: &RouteLine) -> String {
    match route {
        RouteLine::Direct => route_direct_text(),
        RouteLine::Via(name) => route_via_text(name),
        RouteLine::Unavailable(Some(name)) => route_exit_offline_text(OFFLINE_EXIT_MARK, name),
        RouteLine::Unavailable(None) => ROUTE_ALL_OFFLINE_TEXT.to_string(),
    }
}

/// The text of the single node menu item `MENU_NODE` (replaces `MENU_START`/`MENU_STOP`).
/// Its availability is up to the caller — `start_enabled(node) || stop_enabled(node)`.
pub fn node_item_text(node: NodeState) -> &'static str {
    match node {
        NodeState::Absent | NodeState::Unknown => START_TEXT,
        NodeState::Starting | NodeState::Running => STOP_TEXT,
        NodeState::Stopping => NODE_ITEM_STOPPING_TEXT,
    }
}

/// The text of the "Open" menu item by the page state and whether the node has joined a network.
pub fn page_text(view: View) -> String {
    match view.page {
        PageState::Ready if outside_network(view) => net::grey_page_text(view.net),
        PageState::Ready => page_open_text(),
        PageState::Starting => PAGE_STARTING_TEXT.to_string(),
        PageState::Disabled => DISABLED_HINT.to_string(),
        PageState::PortBusy(_) => port_busy_hint_text(),
        PageState::Unavailable => PAGE_UNAVAILABLE_TEXT.to_string(),
    }
}

/// The menu item is clickable only when there is something to open.
pub fn page_enabled(page: PageState) -> bool {
    matches!(page, PageState::Ready)
}

#[path = "view_icon.rs"]
mod icon;
// The `tray_icon_sheet` example includes this file as is and does not call `icon_rgba`.
#[allow(unused_imports)]
pub use icon::icon_rgba;

#[path = "view_unknown.rs"]
mod unknown;

#[path = "view_net.rs"]
mod net;
// The `tray_icon_sheet` example includes this file as is and does not call `net_link`.
#[allow(unused_imports)]
pub use net::net_link;

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "view_words_tests.rs"]
mod words_tests;

#[cfg(test)]
#[path = "view_icon_tests.rs"]
mod icon_tests;

#[cfg(test)]
#[path = "view_status_tests.rs"]
mod status_tests;

#[cfg(test)]
#[path = "view_menu_tests.rs"]
mod menu_tests;
