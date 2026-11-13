//! The tray icon as a ring of sectors, one per network member.
//!
//! What it does: from the node snapshot selection (`TraySnapshot`, `GET /api/state`, `docs/WEB-API.md` §1.3) it computes a `Ring` — how many
//! members are in each role (all but the node itself, with no limit). Roles run clockwise from
//! 12 o'clock: the selected exit is unavailable (red), the exit we go through (blue; dashed when reached through another member), exiting
//! through us (violet), online (green), offline (grey); identical neighbours
//! merge into one arc. The raster is drawn by `view_ring_paint`. Sectors are shown with any icon
//! color if the ring is not empty; an empty ring means a solid icon (`ring_to_draw` gives `None`).
//! What it does not do: it does not draw or place the icon in the tray (`tray/menu_build.rs`) and does not read the network —
//! its only input is `&TraySnapshot`.
//! Links: `Ring` lives in `view::View` (the `View` comparison takes the composition into account); `snapshot_view`
//! (`tray_role/views.rs`) calls `ring_of`; the tray calls `ring_to_draw` and
//! `ring_rgba_lit`.

use crate::web_gui::RouteMode;

use super::snapshot::{TrayPeer, TraySnapshot};
use super::view::View;
pub(super) use super::view_ring_types::role_rgb;
pub use super::view_ring_types::{Ring, RingRole};
#[cfg(test)]
pub(super) use super::view_ring_types::{RING_ROLES, RING_ROLE_COUNT};

impl Ring {
    /// A ring of `count` sectors of one role; `count == 0` gives `EMPTY`.
    pub fn uniform(role: RingRole, count: u16) -> Ring {
        let mut ring = Ring::EMPTY;
        ring.counts[role as usize] = count;
        ring
    }
}

/// The ring from the `/api/state` snapshot: all peers except itself (`self: true`), folded by role, plus
/// the blue sector of the active exit and the red sectors of the unavailable selected exits that are not
/// among the peers. If `peers` is not an array, the ring is empty.
pub fn ring_of(snap: &TraySnapshot) -> Ring {
    let Some(peers) = snap.peers.as_deref() else {
        return Ring::EMPTY;
    };
    let via = active_exit_id_full(snap);
    let down = down_exit_ids_full(snap);
    let facts = RoleFacts {
        via,
        via_busy: active_exit_busy(snap),
        down: &down,
        clients: snap.exit.clients.as_slice(),
    };
    let ring = peers
        .iter()
        .filter_map(|peer| peer_role(peer, &facts))
        .fold(Ring::EMPTY, Ring::add);
    let ring = with_missing_exit(ring, &facts, peers);
    with_missing_down(ring, &down, peers).with_badge(snap.own_exit_overloaded())
}

/// What the role of a member depends on besides the member itself.
struct RoleFacts<'a> {
    /// The `id_full` of the active exit.
    via: Option<&'a str>,
    /// The active exit is overloaded (`route.exits[].busy`).
    via_busy: bool,
    /// The unavailable selected exits.
    down: &'a [&'a str],
    /// Members that go out through us.
    clients: &'a [String],
}

/// Unavailable selected exits that are not among `peers` (the list drifted from the route):
/// each is one red sector. One found among the peers counts by its own role, with no duplicate.
fn with_missing_down(ring: Ring, down: &[&str], peers: &[TrayPeer]) -> Ring {
    let known = |id: &&str| peers.iter().any(|p| p.id_full.as_deref() == Some(*id));
    down.iter()
        .filter(|id| !known(id))
        .fold(ring, |ring, _| ring.add(RingRole::ExitDown))
}

/// The active exit is known from `route`, but it is not among `peers` (the list drifted from the route):
/// still one blue sector, so that the ring doesn't look like "direct". We search all peers,
/// including the node itself; one found (even if offline) counts by its own role, with no duplicate.
fn with_missing_exit(ring: Ring, facts: &RoleFacts, peers: &[TrayPeer]) -> Ring {
    let Some(id) = facts.via else {
        return ring;
    };
    if peers.iter().any(|p| p.id_full.as_deref() == Some(id)) {
        return ring;
    }
    ring.add(exit_role(false, facts.via_busy))
}

/// The active exit says it is overloaded: the same element of `route.exits[]` as `active_exit_id_full` takes.
fn active_exit_busy(snap: &TraySnapshot) -> bool {
    if snap.route.state != RouteMode::Exit {
        return false;
    }
    let exits = &snap.route.exits;
    let marked = exits.iter().find(|e| e.active);
    marked.or(exits.first()).is_some_and(|e| e.busy)
}

/// The `id_full` of the exit we go through: only when `route.state == "exit"`; the element of
/// `route.exits[]` with `active: true`, or the first one if none is marked.
fn active_exit_id_full(snap: &TraySnapshot) -> Option<&str> {
    if snap.route.state != RouteMode::Exit {
        return None;
    }
    let exits = &snap.route.exits;
    let marked = exits.iter().find(|e| e.active);
    marked.or(exits.first())?.id_full.as_deref()
}

/// The `id_full` values of the selected exits when the exit is unavailable: only when `route.state == "unavailable"`,
/// all elements of `route.exits[]`.
fn down_exit_ids_full(snap: &TraySnapshot) -> Vec<&str> {
    if snap.route.state != RouteMode::Unavailable {
        return Vec::new();
    }
    snap.route
        .exits
        .iter()
        .filter_map(|e| e.id_full.as_deref())
        .collect()
}

/// A member's role; `None` means the node itself. A missing `id_full`/`online` does not panic: no `online`
/// means offline.
fn peer_role(peer: &TrayPeer, facts: &RoleFacts) -> Option<RingRole> {
    if peer.is_self {
        return None;
    }
    let relayed = peer.path.as_deref() == Some("relay");
    Some(classify_peer_role(
        peer.id_full.as_deref(),
        peer.online,
        facts,
        relayed,
    ))
}

/// Order of checks: an unavailable selected exit (red beats grey), connection ("offline" is
/// grey even for an exit and a client), exit (blue beats violet; orange when overloaded; dashed when `relayed`),
/// exit client, otherwise online. Another overloaded member stays green: the ring shows roles, not load.
fn classify_peer_role(
    id: Option<&str>,
    online: bool,
    facts: &RoleFacts,
    relayed: bool,
) -> RingRole {
    if id.is_some_and(|id| facts.down.contains(&id)) {
        return RingRole::ExitDown;
    }
    if !online {
        return RingRole::Offline;
    }
    if id.is_some() && id == facts.via {
        return exit_role(relayed, facts.via_busy);
    }
    if id.is_some_and(|id| facts.clients.iter().any(|c| c == id)) {
        return RingRole::ExitClient;
    }
    RingRole::Online
}

/// The role of the active exit: dashed when our path to it goes through another member, orange when it is overloaded.
fn exit_role(relayed: bool, busy: bool) -> RingRole {
    match (relayed, busy) {
        (false, false) => RingRole::ActiveExit,
        (true, false) => RingRole::ExitViaRelay,
        (false, true) => RingRole::ActiveExitBusy,
        (true, true) => RingRole::ExitViaRelayBusy,
    }
}

/// Sectors are needed when the ring has members, with any icon color; otherwise `None` and the icon
/// stays solid (a single node, or the tray cannot see the node).
pub fn ring_to_draw(view: View) -> Option<Ring> {
    (!view.ring.is_empty()).then_some(view.ring)
}

#[cfg(test)]
#[path = "view_ring_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "view_ring_parse_tests.rs"]
mod parse_tests;

#[cfg(test)]
#[path = "view_ring_missing_tests.rs"]
mod missing_tests;

#[cfg(test)]
#[path = "view_ring_busy_tests.rs"]
mod busy_tests;
