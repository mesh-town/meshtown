//! The client's own count of one exit's answers: "does this exit refuse me right now".

use std::time::Instant;

use super::buckets::{index_of, Ring, Tally};

/// Buckets of the window (two minutes).
const WINDOW: u64 = 8;
/// Refusals in the window, at least, to enter.
const ENTER_REFUSED: u64 = 5;
/// Share of refusals among the attempts, percent, at least, to enter.
const ENTER_SHARE: u64 = 20;

/// Attempts and refusals in one bucket.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(super) struct Counts {
    attempts: u64,
    refused: u64,
}

impl Tally for Counts {
    fn add(&mut self, o: &Self) {
        self.attempts += o.attempts;
        self.refused += o.refused;
    }
}

/// What the client got from one exit: the exit answered (any code) or said "busy".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Answer {
    Replied,
    Busy,
}

/// The answers of one exit in 15-second buckets, plus the one bit of state.
pub(crate) struct ExitAnswers {
    origin: Instant,
    ring: Ring<Counts>,
    refusing: bool,
}

impl ExitAnswers {
    pub(crate) fn new(origin: Instant) -> Self {
        Self {
            origin,
            ring: Ring::new(),
            refusing: false,
        }
    }

    pub(crate) fn record(&mut self, answer: Answer, now: Instant) {
        let index = index_of(self.origin, now);
        let bucket = self.ring.at(index);
        bucket.attempts += 1;
        bucket.refused += u64::from(answer == Answer::Busy);
        self.refusing = self.decide(index);
    }

    /// `Some((refused, attempts))` of the window while the exit refuses us.
    pub(crate) fn refusing(&mut self, now: Instant) -> Option<(u64, u64)> {
        let index = index_of(self.origin, now);
        self.refusing = self.decide(index);
        let sum = self.ring.total(index, WINDOW);
        self.refusing.then_some((sum.refused, sum.attempts))
    }

    /// Nothing in the window: the entry holds a record no longer.
    pub(crate) fn is_idle(&self, now: Instant) -> bool {
        self.ring.total(index_of(self.origin, now), WINDOW) == Counts::default()
    }

    /// Entry: enough refusals and a high enough share; once in, stays until a refusal-free window.
    fn decide(&self, index: u64) -> bool {
        let sum = self.ring.total(index, WINDOW);
        match self.refusing {
            true => sum.refused > 0,
            false => {
                sum.refused >= ENTER_REFUSED && sum.refused * 100 >= ENTER_SHARE * sum.attempts
            }
        }
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
