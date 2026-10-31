//! Who holds how many exit streams, and the share of the cap each member may take.
//!
//! A member that asks alone may hold the cap minus a reserve of a fifth, so a newcomer always finds a
//! place. When several members contend, each may hold an equal part of the cap. Contenders are the
//! members that hold a stream, those refused within `CONTEND_WINDOW`, and the member asking now.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::gate::Busy;
use crate::transport::net::PeerId;

/// How long a refused member still counts as a contender.
pub(super) const CONTEND_WINDOW: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Table {
    held: HashMap<PeerId, u64>,
    refused: HashMap<PeerId, Instant>,
}

/// Held places and recent refusals per member. Checks and counts share one lock.
#[derive(Default)]
pub(super) struct Share(Mutex<Table>);

/// The most one member may hold when it contends with `contenders - 1` others (at least 1 contender).
fn ceiling(limit: u64, contenders: u64) -> u64 {
    let reserve = limit - limit / 5;
    match contenders {
        0 | 1 => reserve,
        k => limit.div_ceil(k).max(1).min(reserve.max(1)),
    }
}

impl Table {
    fn forget_stale(&mut self, now: Instant) {
        self.refused
            .retain(|_, at| now.saturating_duration_since(*at) < CONTEND_WINDOW);
    }

    fn contenders(&self, peer: PeerId) -> u64 {
        let mut all: HashSet<PeerId> = self.held.keys().copied().collect();
        all.extend(self.refused.keys().copied());
        all.insert(peer);
        all.len() as u64
    }

    fn held_by(&self, peer: &PeerId) -> u64 {
        self.held.get(peer).copied().unwrap_or(0)
    }
}

impl Share {
    fn lock(&self) -> MutexGuard<'_, Table> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Checks the member's share, then runs `take` (the cap on all streams); on success the member holds one more.
    /// Any refusal keeps the member a contender for the window.
    pub(super) fn admit(
        &self,
        peer: PeerId,
        now: Instant,
        limit: u64,
        take: impl FnOnce() -> Result<(), Busy>,
    ) -> Result<(), Busy> {
        let mut table = self.lock();
        table.forget_stale(now);
        let outcome = Self::check(&table, peer, limit).and_then(|()| take());
        match outcome {
            Ok(()) => *table.held.entry(peer).or_insert(0) += 1,
            Err(_) => {
                table.refused.insert(peer, now);
            }
        }
        outcome
    }

    fn check(table: &Table, peer: PeerId, limit: u64) -> Result<(), Busy> {
        let held = table.held_by(&peer);
        let share = ceiling(limit, table.contenders(peer));
        // A share equal to the whole cap is the cap itself: the stream counter names that refusal.
        match held < share || share >= limit {
            true => Ok(()),
            false => Err(Busy::Share { held, share }),
        }
    }

    /// Notes a refusal that happened after the place was taken (the memory guard).
    pub(super) fn note_refused(&self, peer: PeerId, now: Instant) {
        self.lock().refused.insert(peer, now);
    }

    /// Gives back one place of the member; a member with none left is forgotten.
    pub(super) fn release(&self, peer: &PeerId) {
        let mut table = self.lock();
        let left = table.held_by(peer).saturating_sub(1);
        match left {
            0 => table.held.remove(peer),
            n => table.held.insert(*peer, n),
        };
    }

    #[cfg(test)]
    pub(super) fn len(&self, now: Instant) -> usize {
        let mut table = self.lock();
        table.forget_stale(now);
        table.held.len() + table.refused.len()
    }
}

#[cfg(test)]
#[path = "share_tests.rs"]
mod tests;
