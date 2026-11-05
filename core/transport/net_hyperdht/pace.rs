//! Round pacing of the `hyperdht` source: fast first rounds, then rare ones with jitter (rarer while a gossip neighbor lives), doubling after an error.

use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Pace {
    pub fast_rounds: u32,
    pub fast: Duration,
    pub steady: (Duration, Duration),
    /// Steady-state interval range used while the node has a gossip neighbor.
    pub steady_with_neighbor: (Duration, Duration),
    pub error_first: Duration,
    pub error_max: Duration,
    pub call_limit: Duration,
    pub summary_every: Duration,
}

impl Pace {
    pub fn production() -> Self {
        let s = Duration::from_secs;
        Self {
            fast_rounds: 4,
            fast: s(15),
            steady: (s(120), s(135)),
            steady_with_neighbor: (s(300), s(360)),
            error_first: s(10),
            error_max: s(60),
            call_limit: s(20),
            summary_every: s(600),
        }
    }

    /// Pause after round `round` (from zero); `failures` — errors in a row; `spread` — deterministic jitter;
    /// `has_neighbor` — the node has a gossip neighbor (affects the steady state only).
    pub fn pause(&self, round: u32, failures: u32, spread: u64, has_neighbor: bool) -> Duration {
        if failures > 0 {
            return self.after_error(failures);
        }
        if round < self.fast_rounds {
            return self.fast;
        }
        self.steady_pause(spread, has_neighbor)
    }

    fn after_error(&self, failures: u32) -> Duration {
        let shift = (failures - 1).min(16);
        self.error_first
            .saturating_mul(1u32 << shift)
            .min(self.error_max)
    }

    fn steady_pause(&self, spread: u64, has_neighbor: bool) -> Duration {
        let (lo, hi) = if has_neighbor {
            self.steady_with_neighbor
        } else {
            self.steady
        };
        let width_ms = hi.saturating_sub(lo).as_millis() as u64 + 1;
        lo + Duration::from_millis(spread % width_ms)
    }
}

#[cfg(test)]
#[path = "pace_tests.rs"]
mod tests;
