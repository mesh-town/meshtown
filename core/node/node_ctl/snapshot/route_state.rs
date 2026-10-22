//! Layout of the route decision (`RouteState`) for the `GET /api/state` snapshot.
//!
//! What it does: calls the same pure selection function as the proxy (`route::choose_route`) and moves
//! the result into `RouteState` — the status of each node from `route.exits` (in config order) and the overall
//! mode (`direct`/`exit`/`unavailable`).
//! What it does not do: it does not pick a route again separately from `route::choose_route` — it only formats its
//! result; it does not touch the network.
//! Links: called from `build.rs::build`; the exit list is `Inputs.chosen_exits`, read in
//! `collect.rs` from `node.route.chosen_exits()`.

use crate::transport::net::PeerId;

use crate::node::output;
use crate::proxy::route::{self, Route};
use crate::web_gui::{ChosenExit, PeerInfo, RouteMode, RouteState};

use super::build::Inputs;

/// Builds the snapshot's `route`. `chosen` is the list of chosen exits in `route.exits` order.
pub(super) fn route_state(chosen: &[PeerId], i: &Inputs<'_>, peers: &[PeerInfo]) -> RouteState {
    let decision = route::choose_route(chosen, i.id, &i.members, &i.exits);
    let active = active_exit(&decision);
    let state = route_mode(&decision);
    let exits = chosen
        .iter()
        .map(|&id| chosen_exit(id, i, peers, active))
        .collect();
    RouteState { state, exits }
}

/// The first candidate of the decision (`Route::Exits`) is the only `exits[]` entry with `active: true`.
fn active_exit(decision: &Route) -> Option<PeerId> {
    match decision {
        Route::Exits(list) => Some(route::first_exit(list)),
        Route::Direct | Route::Refuse(_) => None,
    }
}

fn route_mode(decision: &Route) -> RouteMode {
    match decision {
        Route::Direct => RouteMode::Direct,
        Route::Exits(_) => RouteMode::Exit,
        Route::Refuse(_) => RouteMode::Unavailable,
    }
}

/// Status of one node from `route.exits`: the own node is always `online`, the exit flag comes from
/// `i.exit_on`; another node uses the same sources as the `peers[]` rows (`i.members`, `i.exits`).
fn chosen_exit(
    id: PeerId,
    i: &Inputs<'_>,
    peers: &[PeerInfo],
    active: Option<PeerId>,
) -> ChosenExit {
    let id_full = id.to_string();
    let name = peers
        .iter()
        .find(|p| p.id_full == id_full)
        .and_then(|p| p.name.clone());
    let is_self = id == i.id;
    let online = is_self || i.members.contains(&id);
    let exit = if is_self {
        Some(i.exit_on)
    } else if online {
        i.exits.get(&id).copied()
    } else {
        None
    };
    ChosenExit {
        id: output::short_peer_id(&id),
        id_full,
        name,
        online,
        exit,
        country: i.countries.get(&id).cloned(),
        active: active == Some(id),
    }
}

#[cfg(test)]
#[path = "route_state_tests.rs"]
mod tests;
