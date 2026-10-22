//! State of redialing a not-yet-connected member: the last known address, the number
//! of consecutive failures and the time of the next attempt — separately for each id.
//!
//! What it does: for each discovered address decides whether to dial now; on a failure and on
//! a disconnect computes the pause before the next attempt (doubles up to a cap, with a
//! random addition).
//! What it doesn't do: doesn't dial a connection and knows nothing about the transport/`Members` — that is `dial.rs`;
//! takes time as an argument, doesn't read the clock itself (there is no iroh here at all).
//! Links: `dial::PeerDialer` holds one `Redial` for all known ids that aren't connected yet.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::net::{PeerAddr, PeerId};
use tokio::time::Instant;

/// Pause before the first retry attempt.
pub(super) const REDIAL_FIRST: Duration = Duration::from_secs(5);
/// Cap on the pause between attempts.
pub(super) const REDIAL_CAP: Duration = Duration::from_secs(60);
/// Pause after a series of failures (from attempt `REDIAL_LONG_AFTER + 1`): a long-dead peer is knocked on rarely.
pub(super) const REDIAL_LONG: Duration = Duration::from_secs(600);
/// How many consecutive failures get the ordinary pause; after that — `REDIAL_LONG`.
pub(super) const REDIAL_LONG_AFTER: u32 = 8;

/// How many doubling steps still need counting: after that the pause hits `REDIAL_CAP` anyway,
/// and `1u64 << MAX_SHIFT` doesn't overflow for any `attempt`.
const MAX_SHIFT: u32 = 6;

/// Pause before attempt number `attempt` (1 is the first): `REDIAL_FIRST · 2^(attempt-1)` up to
/// `REDIAL_CAP` (from attempt `REDIAL_LONG_AFTER + 1` — `REDIAL_LONG`), plus a random addition of no
/// more than a quarter of the pause (`rnd` is a fraction of `u32::MAX`).
pub(super) fn redial_pause(attempt: u32, rnd: u32) -> Duration {
    let capped_ms = if attempt > REDIAL_LONG_AFTER {
        REDIAL_LONG.as_millis() as u64
    } else {
        let shift = attempt.saturating_sub(1).min(MAX_SHIFT);
        let base_ms = (REDIAL_FIRST.as_millis() as u64).saturating_mul(1u64 << shift);
        base_ms.min(REDIAL_CAP.as_millis() as u64)
    };
    let extra_ms = capped_ms / 4 * u64::from(rnd) / u64::from(u32::MAX);
    Duration::from_millis(capped_ms + extra_ms)
}

/// A loud line — on the first three attempts and on powers of two, quieter after that.
pub(super) fn is_loud(attempt: u32) -> bool {
    attempt <= 3 || attempt.is_power_of_two()
}

/// State of one not-yet-connected id: idle, dial in progress, or waiting until a deadline.
#[derive(Debug, Clone, Copy, Default)]
enum State {
    #[default]
    Idle,
    Dialing,
    Waiting(Instant),
}

impl State {
    /// Wait deadline of the `Waiting` state; the other states have no deadline.
    fn due(self) -> Option<Instant> {
        match self {
            State::Waiting(at) => Some(at),
            State::Idle | State::Dialing => None,
        }
    }
}

/// The last known address, consecutive failures and the state of one id.
#[derive(Debug, Default)]
struct Peer {
    addr: Option<PeerAddr>,
    attempts: u32,
    state: State,
}

/// Decides whether to dial `addr` right now: the id is new and idle, or it is waiting and the address changed (compared
/// with the old address in `peer`, before the caller updates it). A dial is already going its own way — the address
/// will be taken by the next retry; waiting with the same address — the schedule isn't disturbed.
fn should_dial(peer: &Peer, addr: &PeerAddr) -> bool {
    match peer.state {
        State::Idle => true,
        State::Dialing => false,
        State::Waiting(_) => peer.addr.as_ref() != Some(addr),
    }
}

/// Redialing of all known but not yet connected members — state per id.
#[derive(Default)]
pub(super) struct Redial {
    peers: HashMap<PeerId, Peer>,
}

impl Redial {
    /// Remembers the member's address. `Some` with the number of the upcoming attempt (as in `take_due`) —
    /// dial right now (`should_dial`). `None` — a dial is already going (the next retry
    /// will take the address), or it is waiting with the same address — the schedule isn't disturbed. Doesn't
    /// touch the state here: the caller starts the dial right away and advances it itself through `started`/`up`.
    pub(super) fn found(&mut self, addr: PeerAddr) -> Option<(PeerAddr, u32)> {
        let peer = self.peers.entry(addr.id).or_default();
        let fire = should_dial(peer, &addr);
        peer.addr = Some(addr.clone());
        if !fire {
            return None;
        }
        let next_attempt = peer.attempts + 1;
        Some((addr, next_attempt))
    }

    /// Marks that the dial has started (after a successful `Members::try_begin_dial`).
    pub(super) fn started(&mut self, id: PeerId) {
        self.peers.entry(id).or_default().state = State::Dialing;
    }

    /// The member is confirmed: idle, failures to zero, the address stays (useful after a disconnect).
    pub(super) fn up(&mut self, id: PeerId) {
        let peer = self.peers.entry(id).or_default();
        peer.attempts = 0;
        peer.state = State::Idle;
    }

    /// A dial attempt failed: failures plus one, wait until the next deadline. Returns the number
    /// of the failed attempt and the pause computed from it.
    /// `unlisted` — the other side refused us as not admitted yet: the pause never reaches `REDIAL_LONG`, since the
    /// admission record may arrive at any moment.
    pub(super) fn failed(&mut self, id: PeerId, now: Instant, rnd: u32, unlisted: bool) -> (u32, Duration) {
        let peer = self.peers.entry(id).or_default();
        peer.attempts += 1;
        let attempt = peer.attempts;
        let pause = redial_pause(if unlisted { attempt.min(REDIAL_LONG_AFTER) } else { attempt }, rnd);
        peer.state = State::Waiting(now + pause);
        (attempt, pause)
    }

    /// The member vanished: failures to zero (a new series), wait for the first attempt's pause; doesn't
    /// touch the address — if none is remembered, the next `take_due` builds `PeerAddr::new(id)`.
    /// Returns the computed pause.
    pub(super) fn down(&mut self, id: PeerId, now: Instant, rnd: u32) -> Duration {
        let peer = self.peers.entry(id).or_default();
        peer.attempts = 0;
        let pause = redial_pause(1, rnd);
        peer.state = State::Waiting(now + pause);
        pause
    }

    /// The member vanished because of a network change: the attempt is scheduled for `now` with no pause, failures to zero (the next
    /// failure is the ordinary first step `REDIAL_FIRST`). Returns `Duration::ZERO`.
    pub(super) fn down_now(&mut self, id: PeerId, now: Instant) -> Duration {
        let peer = self.peers.entry(id).or_default();
        peer.attempts = 0;
        peer.state = State::Waiting(now);
        Duration::ZERO
    }

    /// Forgets members cut by the prune: entries go away, dialing them stops.
    pub(super) fn forget(&mut self, ids: &HashSet<PeerId>) {
        self.peers.retain(|id, _| !ids.contains(id));
    }

    /// The nearest deadline by which at least one waiting id matures; no waiting ids — `None`.
    pub(super) fn next_due(&self) -> Option<Instant> {
        self.peers.values().filter_map(|p| p.state.due()).min()
    }

    /// All ids whose deadline has come: moves them to "dial in progress", hands out the address (remembered, or
    /// a bare `PeerAddr::new(id)` if the member was only incoming) and the number of the upcoming
    /// attempt.
    pub(super) fn take_due(&mut self, now: Instant) -> Vec<(PeerAddr, u32)> {
        let mut due = Vec::new();
        for (&id, peer) in self.peers.iter_mut() {
            let Some(when) = peer.state.due() else {
                continue;
            };
            if when > now {
                continue;
            }
            let attempt = peer.attempts + 1;
            let addr = peer
                .addr
                .clone()
                .unwrap_or_else(|| PeerAddr::new(id));
            peer.state = State::Dialing;
            due.push((addr, attempt));
        }
        due
    }
}

#[cfg(test)]
#[path = "redial_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "redial_forget_tests.rs"]
mod forget_tests;
