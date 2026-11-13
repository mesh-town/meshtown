//! Keeps the exit's overload record in the network's shared document in line with the meter.
//!
//! What it does: every 15 seconds compares what is needed (overloaded and the exit on) with what lies in the
//! document under `node/exit-load/` and writes, refreshes or clears the record (table in `plan`).
//! What it doesn't do: it doesn't decide overload (the meter does) and doesn't read others' records (`board`).
//! Links: `node::start` runs `run_exit_load_announce` per network next to `run_exit_announce`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;

use super::board::{encode, KEY};
use super::verdict::Reason;
use crate::exit::ExitLimits;
use crate::node::node_state::own_value;
use crate::node::output::say;
use crate::transport::net::{NetError, Replica};

/// How often the state is compared with the document.
const STEP: Duration = Duration::from_secs(15);
/// A live record older than this is rewritten, so readers keep trusting it.
const REFRESH_EVERY: Duration = Duration::from_secs(600);
/// The least time between two writes of the key.
const MIN_WRITE_GAP: Duration = Duration::from_secs(60);

/// What to do with the record on one step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Nothing,
    Write(Reason),
    Clear,
}

/// The table: what is needed against what lies in the document (`have` is `Some(age)` for a live record).
pub(super) fn plan(want: Option<Reason>, have: Option<Duration>) -> Action {
    match (want, have) {
        (Some(r), None) => Action::Write(r),
        (Some(r), Some(age)) if age >= REFRESH_EVERY => Action::Write(r),
        (None, Some(_)) => Action::Clear,
        _ => Action::Nothing,
    }
}

/// The record is needed only while the exit is on and overloaded.
pub(super) fn wanted(exit_on: bool, overloaded: Option<Reason>) -> Option<Reason> {
    overloaded.filter(|_| exit_on)
}

/// The record's age by our own clock; a record "from the future" is as old as zero.
fn age_of(ts_micros: u64, wall_us: u64) -> Duration {
    Duration::from_micros(wall_us.saturating_sub(ts_micros))
}

/// The time of one step: the monotonic clock for the gap, the wall clock for the record's age.
#[derive(Clone, Copy)]
pub(super) struct Tick {
    pub(super) mono: Instant,
    pub(super) wall_us: u64,
}

impl Tick {
    fn now() -> Self {
        let since = SystemTime::now().duration_since(UNIX_EPOCH);
        Self {
            mono: Instant::now(),
            wall_us: since.map_or(0, |d| d.as_micros() as u64),
        }
    }
}

/// The part of the task that survives between steps.
#[derive(Default)]
pub(super) struct Announcer {
    last_write: Option<Instant>,
}

impl Announcer {
    fn gap_passed(&self, now: Instant) -> bool {
        let since = self.last_write.map(|t| now.saturating_duration_since(t));
        since.is_none_or(|d| d >= MIN_WRITE_GAP)
    }

    /// One step. An error is returned to the caller and the next step retries.
    pub(super) async fn step(
        &mut self,
        r: &dyn Replica,
        want: Option<Reason>,
        tick: Tick,
    ) -> Result<Action> {
        let have = own_value(r, KEY).await?;
        let age = have.as_ref().map(|(_, ts)| age_of(*ts, tick.wall_us));
        let action = plan(want, age);
        if action == Action::Nothing || !self.gap_passed(tick.mono) {
            return Ok(Action::Nothing);
        }
        apply(r, action).await?;
        self.last_write = Some(tick.mono);
        log(action, have.is_some());
        Ok(action)
    }
}

async fn apply(r: &dyn Replica, action: Action) -> Result<()> {
    match action {
        Action::Write(reason) => r.set_own(KEY, encode(reason)).await,
        _ => r.del_own(KEY).await,
    }
    .map_err(NetError::into_anyhow)
}

fn log(action: Action, was_live: bool) {
    match action {
        Action::Write(reason) if !was_live => say(
            "exit",
            format!("announced to peers: overloaded ({})", reason.as_str()),
        ),
        Action::Clear => say("exit", "announced to peers: no longer overloaded"),
        _ => {}
    }
}

/// Keeps the record in line with the meter for one network until aborted. It only ticks every `STEP`: it has no
/// alarm of its own, so switching the exit off clears the record on the next step.
pub async fn run_exit_load_announce(
    r: Arc<dyn Replica>,
    exit_on: Arc<AtomicBool>,
    limits: ExitLimits,
) {
    let mut announcer = Announcer::default();
    loop {
        let tick = Tick::now();
        let want = wanted(
            exit_on.load(Ordering::Relaxed),
            limits.overloaded(tick.mono),
        );
        if let Err(e) = announcer.step(r.as_ref(), want, tick).await {
            say("exit", format!("overload record not written: {e:#}"));
        }
        tokio::time::sleep(STEP).await;
    }
}

#[cfg(test)]
#[path = "announce_tests.rs"]
mod tests;
