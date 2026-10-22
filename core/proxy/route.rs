//! Route choice for SOCKS5 connections: direct or through one of the chosen exits.
//!
//! What it does: from the list of chosen exits (`route.exits`, order = priority), the list of
//! online members and the "I am an exit" flags (`node_state::read_exit_flags`) it decides whether to go
//! directly or to ask someone from the list; there is no auto-routing by domain checks any more.
//! What it does not do: it does not open connections, does not consider the request's domain (one decision for all
//! connections at once), and does not move to the next candidate if the chosen one answered with a refusal code.
//! Links: the node's exit list comes from `crate::node::node_state::read_exit_flags` through the document
//! `crate::proxy::domains::DomainList`; members from `crate::members::membership::Members`; the decision is carried out by
//! `crate::proxy` and the console; the same `choose_route` is called by the state snapshot.

use std::collections::{HashMap, HashSet};

use crate::transport::net::PeerId;
use anyhow::Result;

use crate::members::membership::Members;
use crate::node::node_state;
use crate::proxy::domains::DomainList;

/// Route decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// The exit list is empty — connect ourselves.
    Direct,
    /// Online candidates, in `route.exits` order; the list is never empty.
    Exits(Vec<PeerId>),
    /// The exit list is not empty, but no candidate is online; the text is the reason.
    Refuse(&'static str),
}

/// Chooses a route. A pure function, it does not touch the network.
///
/// An empty `chosen_exits` gives `Direct`. Otherwise a candidate is a node from `chosen_exits` that is not `own_id`,
/// is online (`online_peers`) and has the exit flag on (`exit_enabled_by_peer`,
/// no entry = not an exit); repeats of one id in `chosen_exits` count once.
/// No candidates gives `Refuse`.
pub fn choose_route(
    chosen_exits: &[PeerId],
    own_id: PeerId,
    online_peers: &HashSet<PeerId>,
    exit_enabled_by_peer: &HashMap<PeerId, bool>,
) -> Route {
    if chosen_exits.is_empty() {
        return Route::Direct;
    }
    let mut candidates = Vec::new();
    for &id in chosen_exits {
        if id == own_id || candidates.contains(&id) {
            continue;
        }
        if !online_peers.contains(&id) {
            continue;
        }
        if !exit_enabled_by_peer.get(&id).copied().unwrap_or(false) {
            continue;
        }
        candidates.push(id);
    }
    if candidates.is_empty() {
        Route::Refuse("chosen exits are offline")
    } else {
        Route::Exits(candidates)
    }
}

/// First candidate of the `Route::Exits` list — by the `choose_route` contract the list is never empty; the call
/// is needed where the log/console takes exactly the first candidate.
pub fn first_exit(candidates: &[PeerId]) -> PeerId {
    candidates
        .first()
        .copied()
        .expect("Route::Exits is non-empty (choose invariant)")
}

/// Everything needed to choose a route in a running node.
#[derive(Clone)]
pub struct RouteCtx {
    members: Members,
    doc_source: DomainList,
    me: PeerId,
    chosen_exits: std::sync::Arc<tokio::sync::watch::Sender<Vec<PeerId>>>,
}

impl RouteCtx {
    pub fn new(
        members: Members,
        doc_source: DomainList,
        me: PeerId,
        chosen_exits: Vec<PeerId>,
    ) -> RouteCtx {
        let chosen_exits = std::sync::Arc::new(tokio::sync::watch::Sender::new(chosen_exits));
        RouteCtx::with_shared_exits(members, doc_source, me, chosen_exits)
    }

    /// Like `new`, but the exit list is already created outside (the volume probe reads the same one): one
    /// source of the list per node.
    pub fn with_shared_exits(
        members: Members,
        doc_source: DomainList,
        me: PeerId,
        chosen_exits: std::sync::Arc<tokio::sync::watch::Sender<Vec<PeerId>>>,
    ) -> RouteCtx {
        RouteCtx {
            members,
            doc_source,
            me,
            chosen_exits,
        }
    }

    /// Live handle: replaces the list of chosen exits without restarting the node.
    pub fn set_chosen_exits(&self, chosen_exits: Vec<PeerId>) {
        // `send` without subscribers would return an error and not change the value — only
        // `send_replace` fits, as it does not need subscribers.
        self.chosen_exits.send_replace(chosen_exits);
    }

    /// Current list of chosen exits, in priority order.
    pub fn chosen_exits(&self) -> Vec<PeerId> {
        self.chosen_exits.borrow().clone()
    }

    /// Completes when `peer` is no longer in the list of chosen exits — immediately if it is already not
    /// there. A connection through an exit keeps this wait next to `splice`
    /// and closes as soon as the exit disappears from `route.exits`.
    pub async fn wait_until_exit_dropped(&self, peer: PeerId) {
        let mut rx = self.chosen_exits.subscribe();
        // `wait_for` looks at the current value first — if `peer` is already absent, it returns
        // immediately. An error (the sender was dropped together with the node) is also a reason to finish.
        let _ = rx
            .wait_for(|chosen_exits| !chosen_exits.contains(&peer))
            .await;
    }

    /// Chooses a route now. The domain is not involved — one decision for all connections; the same decision
    /// closes an already open connection through an exit if it is removed from the list (`wait_until_exit_dropped`).
    pub async fn choose_now(&self) -> Result<Route> {
        let chosen = self.chosen_exits();
        if chosen.is_empty() {
            return Ok(Route::Direct);
        }
        let online = self.members.ids();
        let serving =
            node_state::read_exit_flags(self.doc_source.shared_replica().as_ref()).await?;
        Ok(choose_route(&chosen, self.me, &online, &serving))
    }
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
