//! Shared exit streams, per exit: a few lanes carry all local gateway connections to that exit.
//!
//! The first lane to an exit is opened by the connection that needs it while the others wait behind a
//! gate, so a burst of connections opens one stream. More lanes (up to `LANES_PER_EXIT`) are opened in
//! the background once every lane carries `SPLIT_AT` connections. A lane without connections is closed
//! after `idle`; a lane that ended for any other reason leaves the map on its own.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use anyhow::Result;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{sleep, Instant};

use super::failure::Failure;
use super::lane::Lane;
use crate::exit::{self, PeerConnPool, EXIT_CODE_OK, UDPGW_HOST, UDPGW_PORT};
use crate::node::output::{say, short_peer_id};
use crate::transport::net::{BoxRecv, BoxSend, NetStack, PeerId};

/// Most shared streams held open to one exit.
pub(super) const LANES_PER_EXIT: usize = 3;
/// Connections per lane (on the least loaded one) from which another lane is opened.
pub(super) const SPLIT_AT: usize = 4;
/// How long a lane without connections stays open.
pub(super) const LANE_IDLE: Duration = Duration::from_secs(30);
/// Pause before another lane is requested from an exit that failed to give one.
pub(super) const LANE_RETRY: Duration = Duration::from_secs(10);

type Pair = (BoxSend, BoxRecv);

/// The lanes of all exits of one pool.
#[derive(Clone)]
pub struct UdpgwMux {
    peers: Arc<Mutex<HashMap<PeerId, PeerLanes>>>,
    idle: Duration,
}

impl Default for UdpgwMux {
    fn default() -> Self {
        UdpgwMux {
            peers: Arc::default(),
            idle: LANE_IDLE,
        }
    }
}

/// The state of one exit.
#[derive(Default)]
struct PeerLanes {
    lanes: Vec<Arc<Lane>>,
    /// Held while the first lane is being opened.
    gate: Arc<AsyncMutex<()>>,
    /// How the last attempt to open the first lane ended, for the connections that waited behind it.
    failure: Option<Failure>,
    /// A background open is in flight.
    extra_opening: bool,
    /// No background open before this moment.
    retry_at: Option<Instant>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl PeerLanes {
    /// Whether the lanes are all loaded and a background open is due.
    fn wants_more(&self, now: Instant) -> bool {
        let loaded = self.lanes.iter().all(|l| l.locals() >= SPLIT_AT);
        let room = self.lanes.len() < LANES_PER_EXIT;
        let allowed = self.retry_at.is_none_or(|t| now >= t);
        loaded && room && allowed && !self.extra_opening
    }

    /// A connection on the least loaded live lane, and whether a background open should start.
    fn take_pair(&mut self) -> Option<(Pair, bool)> {
        self.lanes.retain(|l| !l.is_dead());
        let more = self.wants_more(Instant::now());
        let lane = self.lanes.iter().min_by_key(|l| l.locals())?;
        let pair = lane.attach()?;
        self.extra_opening |= more;
        Some((pair, more))
    }
}

impl UdpgwMux {
    /// The same mux with another idle time (tests).
    #[cfg(test)]
    pub fn with_lane_idle(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    /// How many live lanes lead to `peer` (tests).
    #[cfg(test)]
    pub fn lanes(&self, peer: PeerId) -> usize {
        let peers = lock(&self.peers);
        let Some(state) = peers.get(&peer) else {
            return 0;
        };
        state.lanes.iter().filter(|l| !l.is_dead()).count()
    }

    /// A connection on an existing lane, if the exit has one; may start a background open.
    fn join(&self, stack: &NetStack, pool: &PeerConnPool, peer: PeerId) -> Option<Pair> {
        let taken = lock(&self.peers).get_mut(&peer)?.take_pair();
        let (pair, more) = taken?;
        if more {
            self.open_extra(stack.clone(), pool.clone(), peer);
        }
        Some(pair)
    }

    fn state<R>(&self, peer: PeerId, f: impl FnOnce(&mut PeerLanes) -> R) -> R {
        f(lock(&self.peers).entry(peer).or_default())
    }

    /// Adds a started lane to the exit's map and watches it. Returns the first connection of the lane.
    fn register(&self, peer: PeerId, send: BoxSend, recv: BoxRecv) -> Pair {
        let (lane, first) = Lane::start(send, recv);
        let count = self.state(peer, |s| {
            s.lanes.push(lane.clone());
            s.lanes.len()
        });
        let id = short_peer_id(&peer);
        say(
            "udpgw",
            format!("{id}: shared stream opened ({count} to this exit)"),
        );
        crate::node::output::spawn(self.clone().watch(peer, lane));
        first
    }

    /// Follows a lane until it ends: closes it after `idle` without connections, forgets it once it is dead.
    async fn watch(self, peer: PeerId, lane: Arc<Lane>) {
        let ended = lane.ended();
        tokio::pin!(ended);
        let tick = (self.idle / 4).max(Duration::from_millis(10));
        let mut empty_since: Option<Instant> = None;
        loop {
            tokio::select! {
                () = &mut ended => break,
                () = sleep(tick) => {}
            }
            empty_since = match lane.locals() {
                0 => Some(empty_since.unwrap_or_else(Instant::now)),
                _ => None,
            };
            if empty_since.is_some_and(|t| t.elapsed() >= self.idle) && self.retire(peer, &lane) {
                break;
            }
        }
        self.state(peer, |s| s.lanes.retain(|l| !Arc::ptr_eq(l, &lane)));
    }

    /// Removes an unused lane from the map, then closes it; false if a connection arrived meanwhile.
    fn retire(&self, peer: PeerId, lane: &Arc<Lane>) -> bool {
        let gone = self.state(peer, |s| {
            let unused = lane.locals() == 0;
            if unused {
                s.lanes.retain(|l| !Arc::ptr_eq(l, lane));
            }
            unused
        });
        if gone {
            say(
                "udpgw",
                format!("{}: idle shared stream closed", short_peer_id(&peer)),
            );
            lane.close();
        }
        gone
    }

    fn open_extra(&self, stack: NetStack, pool: PeerConnPool, peer: PeerId) {
        let mux = self.clone();
        crate::node::output::spawn(async move {
            let opened = ask(&stack, &pool, peer).await;
            mux.finish_extra(peer, opened);
        });
    }

    fn finish_extra(&self, peer: PeerId, opened: Result<(BoxSend, BoxRecv, u8)>) {
        let failed = match opened {
            Ok((send, recv, EXIT_CODE_OK)) => {
                drop(self.register(peer, send, recv));
                None
            }
            Ok((_, _, code)) => Some(format!("the exit answered code {code}")),
            Err(e) => Some(format!("{e:#}")),
        };
        if let Some(why) = &failed {
            let id = short_peer_id(&peer);
            say(
                "udpgw",
                format!("{id}: extra shared stream not opened: {why}"),
            );
        }
        self.state(peer, |s| {
            s.extra_opening = false;
            s.retry_at = failed.map(|_| Instant::now() + LANE_RETRY);
        });
    }
}

async fn ask(
    stack: &NetStack,
    pool: &PeerConnPool,
    peer: PeerId,
) -> Result<(BoxSend, BoxRecv, u8)> {
    exit::ask_exit_to_connect(stack, pool, peer, UDPGW_HOST, UDPGW_PORT).await
}

/// A connection to the UDP gateway of `peer` over a shared stream, with the exit's reply code.
/// A code other than OK comes back as the exit gave it and opens no lane.
pub(crate) async fn attach(
    stack: &NetStack,
    pool: &PeerConnPool,
    peer: PeerId,
) -> Result<(BoxSend, BoxRecv, u8)> {
    let mux = &pool.udpgw;
    if let Some((send, recv)) = mux.join(stack, pool, peer) {
        return Ok((send, recv, EXIT_CODE_OK));
    }
    let gate = mux.state(peer, |s| s.gate.clone());
    let (_held, waited) = match gate.clone().try_lock_owned() {
        Ok(held) => (held, false),
        Err(_) => (gate.lock_owned().await, true),
    };
    if let Some((send, recv)) = mux.join(stack, pool, peer) {
        return Ok((send, recv, EXIT_CODE_OK));
    }
    if let Some(failure) = mux.state(peer, |s| s.failure.clone()).filter(|_| waited) {
        return failure.answer();
    }
    open_first(mux, stack, pool, peer).await
}

/// Opens the first lane of an exit; a failure is kept for the connections that waited for it.
async fn open_first(
    mux: &UdpgwMux,
    stack: &NetStack,
    pool: &PeerConnPool,
    peer: PeerId,
) -> Result<(BoxSend, BoxRecv, u8)> {
    let answer = ask(stack, pool, peer).await;
    let failure = match &answer {
        Ok((_, _, EXIT_CODE_OK)) => None,
        Ok((_, _, code)) => Some(Failure::Code(*code)),
        Err(e) => Some(Failure::of(e)),
    };
    mux.state(peer, |s| s.failure = failure);
    let (send, recv, code) = answer?;
    if code != EXIT_CODE_OK {
        return Ok((send, recv, code));
    }
    let (send, recv) = mux.register(peer, send, recv);
    Ok((send, recv, code))
}

#[cfg(test)]
#[path = "lanes_tests.rs"]
mod tests;
