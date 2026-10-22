//! The tray icon as a ring of sectors, one per network member.
//!
//! What it does: from the node snapshot selection (`TraySnapshot`, `GET /api/state`, `docs/WEB-API.md` §1.3) it computes a `Ring` — how many
//! members are in each role (all but the node itself, with no limit). Roles run clockwise from
//! 12 o'clock: the selected exit is unavailable (red), the exit we go through (blue), exiting
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

/// A member's role in the ring. The order of the variants is the index into `Ring::counts` and the order of the sectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingRole {
    /// The selected exit is unavailable (`route.state == unavailable`).
    ExitDown,
    /// The exit our sites go through.
    ActiveExit,
    /// A member that goes out to the network through us.
    ExitClient,
    /// Online.
    Online,
    /// Offline.
    Offline,
}

/// All roles in the order of the variants.
pub(super) const RING_ROLES: [RingRole; RING_ROLE_COUNT] = [
    RingRole::ExitDown,
    RingRole::ActiveExit,
    RingRole::ExitClient,
    RingRole::Online,
    RingRole::Offline,
];

/// The number of roles — the length of `RING_ROLES` and `Ring::counts`.
pub(super) const RING_ROLE_COUNT: usize = 5;

/// The sector color of a role — the only `match` on `RingRole` in the code.
pub(super) fn role_rgb(role: RingRole) -> [u8; 3] {
    match role {
        // The same red as the solid icon `TrayColor::Red` (`view_icon.rs`).
        RingRole::ExitDown => [225, 60, 50],
        // Light blue: on a dark menu row the contrast is about 6:1 (it was about 3.6:1), on a light one about 2:1 — no paler than
        // the green neighbour (1.8:1); from violet ΔE is about 38.
        RingRole::ActiveExit => [90, 170, 255],
        RingRole::ExitClient => [175, 75, 205],
        RingRole::Online => [60, 200, 90],
        RingRole::Offline => [140, 140, 140],
    }
}

/// How many members are in each role (by `RingRole as usize`). It does not remember the order of peers in the snapshot:
/// equality does not depend on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ring {
    counts: [u16; RING_ROLE_COUNT],
}

impl Ring {
    /// An empty ring: the icon stays solid.
    pub const EMPTY: Ring = Ring {
        counts: [0; RING_ROLE_COUNT],
    };

    /// A ring of `count` sectors of one role; `count == 0` gives `EMPTY`.
    pub fn uniform(role: RingRole, count: u16) -> Ring {
        let mut counts = [0; RING_ROLE_COUNT];
        counts[role as usize] = count;
        Ring { counts }
    }

    /// A ring with one more member in role `role`; the counter saturates at `u16::MAX`.
    pub fn add(self, role: RingRole) -> Ring {
        let mut counts = self.counts;
        counts[role as usize] = counts[role as usize].saturating_add(1);
        Ring { counts }
    }

    /// How many members are in the role.
    pub fn count(&self, role: RingRole) -> u16 {
        self.counts[role as usize]
    }

    /// Total members (sectors, counted).
    pub fn len(&self) -> usize {
        self.counts.iter().map(|&n| usize::from(n)).sum()
    }

    /// No members at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The role of the `k`-th member in sector order; `k` past the end gives `Offline`, with no panic.
    pub fn role_at(&self, k: usize) -> RingRole {
        let mut end = 0;
        RING_ROLES
            .into_iter()
            .find(|&role| {
                end += usize::from(self.count(role));
                k < end
            })
            .unwrap_or(RingRole::Offline)
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
    let clients = snap.exit.clients.as_slice();
    let ring = peers
        .iter()
        .filter_map(|peer| peer_role(peer, via, &down, clients))
        .fold(Ring::EMPTY, Ring::add);
    with_missing_down(with_missing_exit(ring, via, peers), &down, peers)
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
fn with_missing_exit(ring: Ring, via: Option<&str>, peers: &[TrayPeer]) -> Ring {
    let Some(id) = via else {
        return ring;
    };
    if peers.iter().any(|p| p.id_full.as_deref() == Some(id)) {
        return ring;
    }
    ring.add(RingRole::ActiveExit)
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
fn peer_role(
    peer: &TrayPeer,
    via: Option<&str>,
    down: &[&str],
    clients: &[String],
) -> Option<RingRole> {
    if peer.is_self {
        return None;
    }
    let id = peer.id_full.as_deref();
    Some(classify_peer_role(id, peer.online, (via, down), clients))
}

/// Order of checks: an unavailable selected exit (red beats grey), connection ("offline" is
/// grey even for an exit and a client), exit (blue beats violet), exit client, otherwise online.
/// `exits` is `(active_exit_id, down_exit_ids)`: the active exit and the unavailable selected ones.
fn classify_peer_role(
    id: Option<&str>,
    online: bool,
    exits: (Option<&str>, &[&str]),
    exit_client_ids: &[String],
) -> RingRole {
    let (active_exit_id, down_exit_ids) = exits;
    if id.is_some_and(|id| down_exit_ids.contains(&id)) {
        return RingRole::ExitDown;
    }
    if !online {
        return RingRole::Offline;
    }
    if id.is_some() && id == active_exit_id {
        return RingRole::ActiveExit;
    }
    if id.is_some_and(|id| exit_client_ids.iter().any(|c| c == id)) {
        return RingRole::ExitClient;
    }
    RingRole::Online
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
