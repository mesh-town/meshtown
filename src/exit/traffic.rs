//! Exit traffic speed per peer: how many bytes per second go through the exit.
//!
//! What it does: `ExitTraffic` is the node's registry; for each EXIT connection `watch` spawns a task,
//! once per `TICK` it puts the connection's `sent_bytes + recv_bytes` delta into the registry; `rates_at`
//! returns the sum of samples over `WINDOW` divided by its length — bytes/s per peer and direction.
//! Direction is the connection's side: `ThroughPeer` — our outgoing EXIT connections (we go
//! through the peer), `ThroughUs` — incoming ones (the peer goes through us).
//! What it doesn't do: it doesn't count ports (`PORT`), volume probes, sync, or direct exit; doesn't print
//! lines; doesn't know the roles of the badge ring.
//! Links: `ExitService::handle` calls `watch(.., ThroughUs)`, `PeerConnPool::connect` —
//! `watch(.., ThroughPeer)`; read by `node_ctl::snapshot::collect` through `rates_at`.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::net::{PeerConn, PeerId};
use tokio::time::sleep;

/// The window over which speed is computed.
pub const WINDOW: Duration = Duration::from_secs(10);

/// Sampling step of the `watch` task.
pub const TICK: Duration = Duration::from_secs(1);

/// Whose traffic: ours through a peer as an exit, or a peer's through our exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    ThroughPeer,
    ThroughUs,
}

/// Speed per peer, bytes/s (sent + received).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeerRate {
    pub through_peer_bps: u64,
    pub through_us_bps: u64,
}

type Samples = HashMap<(PeerId, Direction), VecDeque<(Instant, u64)>>;

/// Registry of exit traffic samples; clones share one state.
#[derive(Clone, Default)]
pub struct ExitTraffic(Arc<Mutex<Samples>>);

fn total_bytes(conn: &dyn PeerConn) -> u64 {
    let link = conn.link();
    link.sent_bytes.saturating_add(link.recv_bytes)
}

impl ExitTraffic {
    /// Watches a connection: once per `TICK` puts in the byte delta, on close the last one, and exits.
    pub fn watch(&self, conn: Arc<dyn PeerConn>, dir: Direction) {
        let (traffic, peer, mut prev) = (self.clone(), conn.peer(), total_bytes(&*conn));
        tokio::spawn(async move {
            while traffic.tick(&*conn, peer, dir, &mut prev).await {}
        });
    }

    /// One task step: waits for `TICK`, writes a delta; `false` — the connection is closed, the task is done.
    async fn tick(&self, conn: &dyn PeerConn, peer: PeerId, dir: Direction, prev: &mut u64) -> bool {
        sleep(TICK).await;
        let closed = conn.is_closed();
        let now = total_bytes(conn);
        self.add(peer, dir, now.saturating_sub(*prev), Instant::now());
        *prev = now;
        !closed
    }

    /// Writes a sample and drops this key's samples older than the window: memory doesn't grow without reads.
    pub(super) fn add(&self, peer: PeerId, dir: Direction, bytes: u64, at: Instant) {
        if bytes == 0 {
            return;
        }
        let mut map = self.0.lock().unwrap();
        let queue = map.entry((peer, dir)).or_default();
        queue.push_back((at, bytes));
        let from = at.checked_sub(WINDOW);
        queue.retain(|&(t, _)| from.is_none_or(|from| t > from));
    }

    /// Speeds per peer at moment `now`; peers with no samples in the window are left out of the answer.
    pub fn rates_at(&self, now: Instant) -> HashMap<PeerId, PeerRate> {
        let from = now.checked_sub(WINDOW);
        let mut map = self.0.lock().unwrap();
        map.retain(|_, queue| {
            queue.retain(|&(t, _)| from.is_none_or(|from| t > from));
            !queue.is_empty()
        });
        let mut rates: HashMap<PeerId, PeerRate> = HashMap::new();
        for (&(peer, dir), queue) in map.iter() {
            let bps = queue.iter().map(|&(_, b)| b).sum::<u64>() / WINDOW.as_secs();
            let rate = rates.entry(peer).or_default();
            match dir {
                Direction::ThroughPeer => rate.through_peer_bps = bps,
                Direction::ThroughUs => rate.through_us_bps = bps,
            }
        }
        rates
    }
}

#[cfg(test)]
#[path = "traffic_tests.rs"]
mod tests;
