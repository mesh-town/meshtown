//! Log of the `hyperdht` source: first success, first error, recovery, a summary once per period.

use std::time::{Duration, Instant};

use crate::net::NetError;

pub(crate) struct Tally {
    rounds: u64,
    errors: u64,
    members: usize,
    last_ok: Option<Instant>,
    failing: bool,
    last_summary: Instant,
    every: Duration,
}

impl Tally {
    pub(crate) fn new(now: Instant, summary_every: Duration) -> Self {
        Self {
            rounds: 0,
            errors: 0,
            members: 0,
            last_ok: None,
            failing: false,
            last_summary: now,
            every: summary_every,
        }
    }

    pub(crate) fn record_round_ok(&mut self, members: usize, now: Instant) {
        self.rounds += 1;
        self.members = members;
        self.note_ok(members);
        self.failing = false;
        self.last_ok = Some(now);
    }

    fn note_ok(&self, members: usize) {
        if self.last_ok.is_none() {
            tracing::info!("hyperdht: record published, members {members}");
            return;
        }
        if self.failing {
            tracing::info!("hyperdht: link restored, members {members}");
        }
    }

    pub(crate) fn record_round_failure(&mut self, err: &NetError, _now: Instant) {
        self.rounds += 1;
        self.errors += 1;
        if !self.failing {
            tracing::warn!("hyperdht: round error: {err}");
        }
        self.failing = true;
    }

    pub(crate) fn log_summary_if_due(&mut self, now: Instant) {
        if now.duration_since(self.last_summary) < self.every {
            return;
        }
        self.last_summary = now;
        let since = self.last_ok.map(|t| now.duration_since(t).as_secs());
        tracing::info!(
            "hyperdht: summary: rounds {}, errors {}, members {}, since last success {}",
            self.rounds,
            self.errors,
            self.members,
            since.map_or("—".to_string(), |s| format!("{s} s")),
        );
    }

    #[cfg(test)]
    pub(crate) fn counts(&self) -> (u64, u64) {
        (self.rounds, self.errors)
    }
}

#[cfg(test)]
#[path = "tally_tests.rs"]
mod tests;
