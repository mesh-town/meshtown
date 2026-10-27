//! Which relay a dial to a peer uses: the last one that worked, else the next candidate in turn.

use super::source::RelayCandidate;
use crate::transport::net::id::PeerId;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a peer reached through a relay is dialed through it first, without a direct attempt.
pub(crate) const RELAY_FIRST_TTL: Duration = Duration::from_secs(600);
/// How many peers the book remembers; an arbitrary old one is dropped on overflow.
const MAX_BOOK_PEERS: usize = 256;

/// The choice for one dial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RelayPick {
    pub cand: RelayCandidate,
    /// Try the direct phase before the relay phase.
    pub direct_first: bool,
}

#[derive(Default)]
struct Entry {
    worked: Option<(RelayCandidate, Instant)>,
    cursor: usize,
}

/// Per-peer relay memory. Locks are short and never held across an await.
#[derive(Default)]
pub(crate) struct RelayBook {
    peers: Mutex<HashMap<PeerId, Entry>>,
}

impl RelayBook {
    fn lock(&self) -> MutexGuard<'_, HashMap<PeerId, Entry>> {
        self.peers.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The candidate for a dial to `target`; `list` arrives sorted and is not reordered here.
    pub(crate) fn pick(
        &self,
        target: PeerId,
        list: &[RelayCandidate],
        now: Instant,
    ) -> Option<RelayPick> {
        let peers = self.lock();
        let entry = peers.get(&target);
        let worked = entry
            .and_then(|e| e.worked)
            .filter(|(c, _)| list.contains(c));
        let cursor = entry.map_or(0, |e| e.cursor);
        worked
            .map(|(cand, at)| recent(cand, at, now))
            .or_else(|| turn(list, cursor))
    }

    /// The relay phase through `cand` reached `target`.
    pub(crate) fn succeeded(&self, target: PeerId, cand: RelayCandidate, now: Instant) {
        let mut peers = self.lock();
        make_room(&mut peers, target);
        peers.entry(target).or_default().worked = Some((cand, now));
    }

    /// The relay phase through `cand` failed: forget the success and move on to the next candidate.
    pub(crate) fn failed(&self, target: PeerId, cand: RelayCandidate) {
        let mut peers = self.lock();
        make_room(&mut peers, target);
        let entry = peers.entry(target).or_default();
        let forget = entry.worked.is_some_and(|(c, _)| c == cand);
        entry.worked = entry.worked.filter(|_| !forget);
        entry.cursor = entry.cursor.wrapping_add(1);
    }
}

fn recent(cand: RelayCandidate, at: Instant, now: Instant) -> RelayPick {
    let direct_first = now.saturating_duration_since(at) >= RELAY_FIRST_TTL;
    RelayPick { cand, direct_first }
}

fn turn(list: &[RelayCandidate], cursor: usize) -> Option<RelayPick> {
    let cand = list.get(cursor.checked_rem(list.len())?).copied()?;
    Some(RelayPick {
        cand,
        direct_first: true,
    })
}

fn make_room(peers: &mut HashMap<PeerId, Entry>, target: PeerId) {
    if peers.contains_key(&target) || peers.len() < MAX_BOOK_PEERS {
        return;
    }
    let victim = peers.keys().next().copied();
    victim.iter().for_each(|id| {
        peers.remove(id);
    });
}

#[cfg(test)]
#[path = "book_tests.rs"]
mod tests;
