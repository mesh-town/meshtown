//! The check schedule: a base interval that doubles for an unchanged result (`Schedule`,
//! `DomainState`), the "being checked right now" mark (`InProgress`) and the decision of who is due
//! (`Checks::due`/`schedule_after_verdict`/`postpone`, called from `run.rs`). The schedule is kept per pair
//! (domain, `Target`): a result "directly" does not move the due time "through the exit of P".

use portable_atomic::AtomicU64;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use crate::site_checks::reachability::CheckVerdict;
use crate::transport::net::PeerId;

use super::state::Checks;
use super::target::Target;

/// How long after a domain appears it is first checked.
pub const FIRST_DELAY: Duration = Duration::from_secs(5);
/// Spread of nodes for the first check of a new domain: the first check happens after `FIRST_DELAY` +
/// the node's offset within `FIRST_SPREAD`, not the interval: a new domain should get a result within
/// a minute, while `schedule_after_verdict` spreads nodes over the interval from the second check on.
pub const FIRST_SPREAD: Duration = Duration::from_secs(60);
/// The largest power of two by which the interval of a domain with an unchanged result
/// is stretched: 2⁴ = 16 base intervals; beyond that `Schedule::max` limits it.
pub const MAX_BACKOFF_EXPONENT: u32 = 4;

/// Schedule settings. `base_interval_secs` is a live handle (seconds behind an atomic), `max` is not:
/// the backoff cap is not exposed.
#[derive(Debug, Clone)]
pub struct Schedule {
    base_interval_secs: Arc<AtomicU64>,
    pub max: Duration,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule {
            base_interval_secs: Arc::new(AtomicU64::new(300)),
            max: Duration::from_secs(3600),
        }
    }
}

impl Schedule {
    /// A schedule with the given base interval (replaces a struct literal from outside `checks.rs`).
    pub fn with_base_interval(base: Duration) -> Schedule {
        Schedule {
            base_interval_secs: Arc::new(AtomicU64::new(base.as_secs().max(1))),
            ..Schedule::default()
        }
    }

    /// Reads the base interval; called from `state.rs` and `run.rs`, hence `pub(super)`.
    pub(super) fn base_interval(&self) -> Duration {
        Duration::from_secs(self.base_interval_secs.load(Ordering::Relaxed))
    }

    pub(super) fn set_base_interval(&self, d: Duration) {
        self.base_interval_secs
            .store(d.as_secs().max(1), Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub(super) struct DomainState {
    next_at: Instant,
    last_verdict: Option<CheckVerdict>,
    same_verdict_streak: u32,
}

/// The set of domains being checked right now: a manual check through
/// `try_begin_check`/`end_check` keeps the scheduled loop from taking the same domain at the same time.
#[derive(Default)]
pub(super) struct InProgress(std::sync::Mutex<HashSet<String>>);

impl InProgress {
    /// Called from `Checks::try_begin_check` in `state.rs`, hence `pub(super)`.
    pub(super) fn try_begin(&self, domain: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(domain.to_string())
    }

    pub(super) fn end(&self, domain: &str) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(domain);
    }

    pub(super) fn domains_in_progress(&self) -> HashSet<String> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// The node's constant offset within the interval: each author has its own, and it doesn't change between rounds.
fn offset_for(author: crate::transport::net::PeerId, domain: &str, interval: Duration) -> Duration {
    let mut hasher = blake3::Hasher::new();
    hasher.update(author.as_bytes());
    hasher.update(domain.as_bytes());
    let digest = hasher.finalize();
    let bytes: [u8; 4] = digest.as_bytes()[..4]
        .try_into()
        .expect("4 bytes of the hash");
    let seconds = interval.as_secs().max(1);
    Duration::from_secs(u64::from(u32::from_le_bytes(bytes)) % seconds)
}

/// How long after a domain appears it is first checked: `FIRST_DELAY` and the constant
/// node offset within `FIRST_SPREAD`; it doesn't depend on the base interval.
fn first_check_delay(author: crate::transport::net::PeerId, domain: &str) -> Duration {
    FIRST_DELAY + offset_for(author, domain, FIRST_SPREAD)
}

/// The schedule states of all (domain, target) pairs.
#[derive(Debug, Default)]
pub(super) struct DomainStates(HashMap<(String, Target), DomainState>);

impl DomainStates {
    /// Whether the pair is due at `now`; a new pair gets its first check scheduled after the first-check delay.
    pub(super) fn is_due(
        &mut self,
        domain: &str,
        target: Target,
        now: Instant,
        me: PeerId,
    ) -> bool {
        let state = self
            .0
            .entry((domain.to_string(), target))
            .or_insert_with(|| DomainState {
                next_at: now + first_check_delay(me, domain),
                last_verdict: None,
                same_verdict_streak: 0,
            });
        state.next_at <= now
    }

    /// Remembers the verdict of the pair and schedules the next check after the backed-off interval.
    pub(super) fn record(
        &mut self,
        (domain, target): (&str, Target),
        verdict: CheckVerdict,
        now: Instant,
        (schedule, me): (&Schedule, PeerId),
    ) {
        let entry = self
            .0
            .entry((domain.to_string(), target))
            .or_insert_with(|| DomainState {
                next_at: now,
                last_verdict: None,
                same_verdict_streak: 0,
            });
        entry.remember_verdict(verdict);
        let interval = schedule.backoff_interval(entry.same_verdict_streak);
        entry.next_at = now + interval + offset_for(me, domain, interval);
    }

    fn postpone(&mut self, domain: &str, target: Target, until: Instant) {
        if let Some(entry) = self.0.get_mut(&(domain.to_string(), target)) {
            entry.next_at = until;
        }
    }
}

impl DomainState {
    /// Counts a run of equal verdicts (capped at `MAX_BACKOFF_EXPONENT`); a different verdict restarts it.
    fn remember_verdict(&mut self, verdict: CheckVerdict) {
        if self.last_verdict == Some(verdict) {
            self.same_verdict_streak = (self.same_verdict_streak + 1).min(MAX_BACKOFF_EXPONENT);
        } else {
            self.same_verdict_streak = 0;
        }
        self.last_verdict = Some(verdict);
    }
}

impl Schedule {
    /// The base interval doubled `streak` times, capped at `max`.
    pub(super) fn backoff_interval(&self, streak: u32) -> Duration {
        (self.base_interval() * (1 << streak)).min(self.max)
    }
}

impl Checks {
    /// Whether it is time to check the domain through the target; also creates the state for a new pair.
    pub(super) async fn due(&self, domain: &str, target: Target) -> bool {
        let mut states = self.domain_states.lock().await;
        states.is_due(domain, target, Instant::now(), self.shared_replica.me())
    }

    pub(super) async fn schedule_after_verdict(
        &self,
        domain: &str,
        target: Target,
        verdict: CheckVerdict,
    ) {
        let mut states = self.domain_states.lock().await;
        let me = self.shared_replica.me();
        states.record(
            (domain, target),
            verdict,
            Instant::now(),
            (&self.schedule, me),
        );
    }

    /// Postpones the pair by one base interval, remembering nothing.
    pub(super) async fn postpone(&self, domain: &str, target: Target) {
        let until = Instant::now() + self.schedule.base_interval();
        self.domain_states
            .lock()
            .await
            .postpone(domain, target, until);
    }
}

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;
