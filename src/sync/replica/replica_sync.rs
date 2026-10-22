//! The replica's entry into the network: the live broadcast topic, `start_sync`, `sync_with`, broadcasting its own entry.
//! Reconciliation sessions are in `replica_session.rs`, topic events are in `replica_swarm.rs`.

use super::entry::WireEntry;
use super::ids::topic_of;
use super::replica::MeshReplica;
use super::wire::encode_push;
use crate::net::error::{NetError, NetResult};
use crate::net::id::PeerId;
use crate::net::sync_backend::{SyncBackend, REPLICA_DOMAINS};
use crate::net::{Membership, SwarmEvent};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;
use tokio::sync::{broadcast, OnceCell};
use tokio::time::Instant;

/// How long `sync_with` waits for a peer to become a topic peer before dialing without that.
const NEIGHBOR_WAIT: Duration = Duration::from_secs(3);

/// How often a replica with a topic reconciles fully with its neighbors, in case a live broadcast was lost.
const RESYNC_EVERY: Duration = Duration::from_secs(120);

/// The replica's network part: the topic, sessions in flight, peers, authors with noticed "far-ahead" clocks.
#[derive(Default)]
pub(super) struct NetState {
    pub topic_membership: OnceCell<Arc<dyn Membership>>,
    peers_in_session: Mutex<HashSet<PeerId>>,
    neighbors: Mutex<HashSet<PeerId>>,
    warned_authors: Mutex<HashSet<PeerId>>,
}

/// The slot of a session with a peer; while it is alive, a second session with that peer does not start.
pub(super) struct SessionGuard {
    owner: Arc<MeshReplica>,
    peer: PeerId,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        lock(&self.owner.net.peers_in_session).remove(&self.peer);
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl MeshReplica {
    pub(super) async fn start_sync_impl(&self) -> NetResult<()> {
        if self.name == REPLICA_DOMAINS {
            return Err(NetError::Refused);
        }
        self.net.topic_membership.get_or_try_init(|| self.open_topic()).await.map(|_| ())
    }

    /// Joins the replica's broadcast topic and starts the pump of its events.
    async fn open_topic(&self) -> NetResult<Arc<dyn Membership>> {
        let flood = self.wiring.flood.get().and_then(Weak::upgrade).ok_or(NetError::Closed)?;
        let swarm = flood.join_swarm(topic_of(&self.id), &[]).await?;
        tokio::spawn(pump(self.this.clone(), swarm.events));
        Ok(swarm.membership)
    }

    /// `Ok` means "started": the result will arrive as a `Synced` event. A session with this peer is already running — `Ok` without a new one.
    pub(super) async fn sync_with_impl(&self, peer: PeerId) -> NetResult<()> {
        if self.name == REPLICA_DOMAINS {
            return Err(NetError::Refused);
        }
        let owner = self.this.upgrade().ok_or(NetError::Closed)?;
        let Some(slot) = owner.claim(peer) else { return Ok(()) };
        self.start_sync_impl().await?;
        let membership = self.net.topic_membership.get().cloned().ok_or(NetError::Closed)?;
        let mut events = membership.subscribe();
        membership.join(&[peer]).await?;
        tokio::spawn(async move {
            owner.wait_neighbor(peer, &mut events).await;
            owner.dial_session(peer, slot).await;
        });
        Ok(())
    }

    /// Takes the session slot with `peer`; `None` — it is already taken.
    pub(super) fn claim(self: &Arc<Self>, peer: PeerId) -> Option<SessionGuard> {
        lock(&self.net.peers_in_session).insert(peer).then(|| SessionGuard { owner: self.clone(), peer })
    }

    /// Waits for `NeighborUp(peer)` for no longer than `NEIGHBOR_WAIT`: then the broadcast after reconciliation already arrives.
    async fn wait_neighbor(&self, peer: PeerId, events: &mut broadcast::Receiver<SwarmEvent>) {
        let waiting = async {
            while !self.is_neighbor(peer) {
                let got = events.recv().await;
                if matches!(got, Err(broadcast::error::RecvError::Closed)) {
                    return;
                }
                if matches!(got, Ok(SwarmEvent::NeighborUp(p)) if p == peer) {
                    return;
                }
            }
        };
        let _ = tokio::time::timeout(NEIGHBOR_WAIT, waiting).await;
    }

    pub(super) fn is_neighbor(&self, peer: PeerId) -> bool {
        lock(&self.net.neighbors).contains(&peer)
    }

    pub(super) fn set_neighbor(&self, peer: PeerId, up: bool) {
        let mut set = lock(&self.net.neighbors);
        let _ = if up { set.insert(peer) } else { set.remove(&peer) };
    }

    pub(super) fn neighbor_list(&self) -> Vec<PeerId> {
        lock(&self.net.neighbors).iter().copied().collect()
    }

    /// `true` only the first time for an author: the "clock far ahead" log once per author.
    pub(super) fn first_warning(&self, author: PeerId) -> bool {
        lock(&self.net.warned_authors).insert(author)
    }

    /// A live broadcast of a new entry to the topic's peers; no topic — nothing.
    pub(super) fn push_entry(&self, w: WireEntry) {
        let Some(membership) = self.net.topic_membership.get().cloned() else { return };
        let Ok(body) = encode_push(&self.id, vec![w]) else { return };
        tokio::spawn(async move {
            let _ = membership.broadcast(body).await;
        });
    }
}

/// Passes the topic events to the replica while it is alive, and reconciles with neighbors on a timer.
async fn pump(replica: Weak<MeshReplica>, mut events: broadcast::Receiver<SwarmEvent>) {
    let mut next = Instant::now() + resync_delay(random_u32());
    loop {
        let ev = tokio::select! {
            got = events.recv() => match got {
                Ok(ev) => ev,
                Err(broadcast::error::RecvError::Lagged(_)) => SwarmEvent::Lagged,
                Err(broadcast::error::RecvError::Closed) => return,
            },
            () = tokio::time::sleep_until(next) => {
                next = Instant::now() + resync_delay(random_u32());
                let Some(owner) = replica.upgrade() else { return };
                owner.resync_smaller();
                continue;
            }
        };
        let Some(owner) = replica.upgrade() else { return };
        owner.on_swarm(ev);
    }
}

/// Pause before the next periodic reconciliation: `RESYNC_EVERY` plus a random addition of up to a quarter.
pub(super) fn resync_delay(rnd: u32) -> Duration {
    RESYNC_EVERY + RESYNC_EVERY / 4 * rnd / u32::MAX
}

fn random_u32() -> u32 {
    let mut bytes = [0u8; 4];
    rand::fill(&mut bytes);
    u32::from_le_bytes(bytes)
}
