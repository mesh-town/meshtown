//! Network members and gossip events.
//!
//! What it does: counts live verified connections and outgoing dial attempts in progress,
//! holds a connection until it closes (volume probe in the background), parses events of the topic
//! gossip.
//! What it doesn't do: it neither opens connections nor checks the secret — `handshake` does that.
//! Links: `handshake::initiate`/`respond` call `Members::add_verified_conn`; `node::start_node` starts
//! `run_swarm_events` and handles `MemberEvent` from the `MemberContext::events` channel.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::access::view::{AccessView, Deny, ACCESS_CLOSE_CODE};
use crate::net::{PeerConn, PeerId, SwarmEvent};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc};

use crate::output::{say, short_peer_id};
use crate::transfer_probe::{self, ProbeSettings};

/// State under a lock: member connection counters, dials in progress and member names.
#[derive(Default)]
struct MembersState {
    conn_count_by_peer: HashMap<PeerId, usize>,
    dialing: HashSet<PeerId>,
    names: HashMap<PeerId, String>,
    /// Live member connections by `stable_id` — for counting connections and paths in `[status]`.
    conns: HashMap<u64, Arc<dyn PeerConn>>,
}

impl std::fmt::Debug for MembersState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MembersState")
            .field("conn_count_by_peer", &self.conn_count_by_peer)
            .field("dialing", &self.dialing)
            .field("names", &self.names)
            .field("conns", &self.conns.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Number of live verified connections per member and the set of dial attempts in progress.
#[derive(Clone, Default, Debug)]
pub struct Members {
    state: Arc<Mutex<MembersState>>,
    access: AccessView,
}

impl Members {
    /// Counts a new verified connection. Returns `true` if the member became new (0 → 1).
    pub fn add_verified_conn(&self, id: PeerId) -> bool {
        let mut guard = self.state.lock().unwrap();
        let counter = guard.conn_count_by_peer.entry(id).or_insert(0);
        *counter += 1;
        let first = *counter == 1;
        drop(guard);
        if first {
            self.access.clear_refusal(id);
        }
        first
    }

    /// Removes one verified connection. Returns `true` if the member disappeared (1 → 0). It does not erase the name —
    /// if the peer returns, the line will show the same name again.
    pub fn remove_verified_conn(&self, id: PeerId) -> bool {
        let mut guard = self.state.lock().unwrap();
        if let Some(counter) = guard.conn_count_by_peer.get_mut(&id) {
            if *counter > 0 {
                *counter -= 1;
            }
            if *counter == 0 {
                guard.conn_count_by_peer.remove(&id);
                return true;
            }
        }
        false
    }

    pub fn is_member(&self, id: PeerId) -> bool {
        self.access.allows(id) && self.state.lock().unwrap().conn_count_by_peer.contains_key(&id)
    }

    /// Members whose list verdicts come from `view`.
    pub fn with_access(view: AccessView) -> Members {
        Members { state: Arc::default(), access: view }
    }

    pub fn access(&self) -> AccessView {
        self.access.clone()
    }

    /// Closes every tracked connection of the peer with the refusal code; returns how many were closed.
    pub fn cut(&self, peer: PeerId, deny: Deny) -> usize {
        let conns = self.conns_snapshot();
        let of_peer = conns.iter().filter(|c| c.peer() == peer);
        of_peer.map(|c| c.close(ACCESS_CLOSE_CODE, deny.wire_reason())).count()
    }

    /// List of members right now — path choice needs it to avoid calling someone with no connection.
    pub fn ids(&self) -> HashSet<PeerId> {
        self.state.lock().unwrap().conn_count_by_peer.keys().copied().collect()
    }

    pub fn count(&self) -> usize {
        self.state.lock().unwrap().conn_count_by_peer.len()
    }

    /// Remembers a member's live connection (`hold_conn_until_closed` holds it).
    pub fn track_conn(&self, conn: &Arc<dyn PeerConn>) {
        self.state.lock().unwrap().conns.insert(conn.stable_id(), conn.clone());
    }

    /// Closes the connection if the list denies the peer (a race with `cut`: the list changed before tracking).
    fn refuse_if_denied(&self, conn: &Arc<dyn PeerConn>, peer: PeerId) {
        let denied = self.access.check(peer).err();
        denied.iter().for_each(|deny| conn.close(ACCESS_CLOSE_CODE, deny.wire_reason()));
    }

    /// Forgets a closed connection.
    pub fn untrack_conn(&self, conn: &Arc<dyn PeerConn>) {
        self.state.lock().unwrap().conns.remove(&conn.stable_id());
    }

    /// Snapshot of members' live connections (cloning an `Arc` is cheap); the lock is released before returning,
    /// so walking and formatting do not hold it (memory guard).
    pub fn conns_snapshot(&self) -> Vec<Arc<dyn PeerConn>> {
        self.state.lock().unwrap().conns.values().cloned().collect()
    }

    /// The `[status]` line: how many live member connections and how many paths in them.
    pub fn conns_text(&self) -> String {
        let conns = self.conns_snapshot();
        let paths: usize = conns.iter().map(|c| c.link().path_count).sum();
        format!("connections {}, paths {paths}", conns.len())
    }

    /// Returns `false` if the member is already present or a dial attempt is already in progress; otherwise remembers the attempt.
    pub fn try_begin_dial(&self, id: PeerId) -> bool {
        let mut guard = self.state.lock().unwrap();
        if !self.access.allows(id) || guard.conn_count_by_peer.contains_key(&id) || guard.dialing.contains(&id) {
            return false;
        }
        guard.dialing.insert(id);
        true
    }

    pub fn end_dial(&self, id: PeerId) {
        self.state.lock().unwrap().dialing.remove(&id);
    }

    /// Inserts or overwrites a member's name (the peer restarted with another name — the
    /// log shows the new one).
    pub fn set_name(&self, id: PeerId, name: String) {
        self.state.lock().unwrap().names.insert(id, name);
    }

    /// A member's name, if known.
    pub fn name_of(&self, id: PeerId) -> Option<String> {
        self.state.lock().unwrap().names.get(&id).cloned()
    }

    /// A copy of the map of known member names — under the same lock, a snapshot at the moment of the call
    /// (the node snapshot `collect` needs all names at once, not one by one through `name_of`).
    pub fn names(&self) -> HashMap<PeerId, String> {
        self.state.lock().unwrap().names.clone()
    }

    /// `<short id> "<name>"`, or just the short id if there is no name.
    pub fn peer_label(&self, id: PeerId) -> String {
        match self.name_of(id) {
            Some(name) => format!("{} \"{name}\"", short_peer_id(&id)),
            None => short_peer_id(&id),
        }
    }
}

/// Event of a change in the set of members.
pub enum MemberEvent {
    Up { peer: PeerId, conn: Arc<dyn PeerConn> },
    Down { peer: PeerId, reason: String },
}

/// Shared member context: own id, handshake key, member bookkeeping, event channel,
/// volume probe settings.
#[derive(Clone)]
pub struct MemberContext {
    pub own_id: PeerId,
    pub handshake_key: [u8; 32],
    pub members: Members,
    pub events: mpsc::UnboundedSender<MemberEvent>,
    pub probe: ProbeSettings,
    /// The current own name: read by handshakes (goes to the peer), awaited by the document publisher,
    /// written by `RunningNode::set_node_name`.
    pub own_name_tx: tokio::sync::watch::Sender<String>,
}

impl std::fmt::Debug for MemberContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberContext")
            .field("me", &self.own_id)
            .field("members", &self.members)
            .field("probe", &self.probe)
            .field("name", &*self.own_name_tx.borrow())
            .finish()
    }
}

/// Holds a verified connection alive: volume probe in the background, waiting for close,
/// removal of the member and a `Down` event on close.
pub async fn hold_conn_until_closed(conn: Arc<dyn PeerConn>, peer: PeerId, ctx: MemberContext) {
    let serve_handle = tokio::spawn(transfer_probe::answer_probes(conn.clone(), peer));
    let drive_handle = tokio::spawn(transfer_probe::send_probes_while_open(conn.clone(), peer, ctx.probe.clone()));
    ctx.members.track_conn(&conn);
    ctx.members.refuse_if_denied(&conn, peer);
    conn.closed().await;
    let reason = conn.close_reason();
    ctx.members.untrack_conn(&conn);
    serve_handle.abort();
    drive_handle.abort();
    if ctx.members.remove_verified_conn(peer) {
        let _ = ctx.events.send(MemberEvent::Down { peer, reason });
    }
}

/// Parses gossip topic events and keeps the count of current peers.
pub async fn run_swarm_events(
    mut events: broadcast::Receiver<SwarmEvent>,
    gossip_neighbor_count: Arc<AtomicUsize>,
) {
    let mut peers: HashSet<PeerId> = HashSet::new();
    loop {
        let keep_going = match events.recv().await {
            Ok(event) => on_swarm_event(event, &mut peers, &gossip_neighbor_count),
            Err(RecvError::Lagged(_)) => {
                say("gossip", "events skipped");
                true
            }
            Err(RecvError::Closed) => false,
        };
        if !keep_going {
            break;
        }
    }
}

/// Parses one topic event; `false` — the topic is closed, the loop ends.
fn on_swarm_event(event: SwarmEvent, peers: &mut HashSet<PeerId>, gossip_neighbor_count: &AtomicUsize) -> bool {
    match event {
        SwarmEvent::NeighborUp(id) => {
            peers.insert(id);
            gossip_neighbor_count.store(peers.len(), Ordering::Relaxed);
            say("gossip", format!("peer + {}", short_peer_id(&id)));
        }
        SwarmEvent::NeighborDown(id) => {
            peers.remove(&id);
            gossip_neighbor_count.store(peers.len(), Ordering::Relaxed);
            say("gossip", format!("peer - {}", short_peer_id(&id)));
        }
        SwarmEvent::Received { from, msg } => {
            say(
                "gossip",
                format!(
                    "message from {}: {}",
                    short_peer_id(&from),
                    String::from_utf8_lossy(&msg)
                ),
            );
        }
        SwarmEvent::Lagged => say("gossip", "events skipped"),
        SwarmEvent::Closed(reason) => {
            say("gossip", format!("topic error: {reason}"));
            return false;
        }
    }
    true
}

#[cfg(test)]
#[path = "membership_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "membership_swarm_tests.rs"]
mod swarm_tests;
