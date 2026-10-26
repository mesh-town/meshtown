//! A rate limiter for repeated log lines: one line per interval, with the number of lines held back.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Refusals are written to the log at most this often.
const LINE_EVERY: Duration = Duration::from_secs(10);

/// Time of the last line and the number of events held back since.
#[derive(Default)]
pub(crate) struct LineRate(Mutex<(Option<Instant>, u64)>);

impl LineRate {
    /// `Some(events held back since the last line)` when a line is due, else `None`
    /// (the event is counted for the next line).
    pub(crate) fn due(&self, now: Instant) -> Option<u64> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let (last, held) = &mut *state;
        let due = last.is_none_or(|at| now.saturating_duration_since(at) >= LINE_EVERY);
        match due {
            true => {
                *last = Some(now);
                Some(std::mem::take(held))
            }
            false => {
                *held += 1;
                None
            }
        }
    }
}
