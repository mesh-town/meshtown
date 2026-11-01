//! A rate limiter for repeated log lines: one line per interval, with the number of lines held back.

use std::collections::HashMap;
use std::hash::Hash;
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

/// Past this many keys, entries that have nothing to report and are older than the window are dropped.
const PRUNE_FROM: usize = 64;

/// The same limiter with its own window and held-back count per key (a member, a member and a port).
pub(crate) struct KeyedLineRate<K>(Mutex<HashMap<K, (Instant, u64)>>);

impl<K> Default for KeyedLineRate<K> {
    fn default() -> Self {
        Self(Mutex::default())
    }
}

impl<K: Eq + Hash + Copy> KeyedLineRate<K> {
    /// `Some(events of this key held back since its last line)` when a line is due, else `None`
    /// (the event is counted for the next line of this key).
    pub(crate) fn due(&self, key: K, now: Instant) -> Option<u64> {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() >= PRUNE_FROM {
            map.retain(|_, (at, held)| {
                *held > 0 || now.saturating_duration_since(*at) < LINE_EVERY
            });
        }
        match map.get_mut(&key) {
            Some((at, held)) if now.saturating_duration_since(*at) < LINE_EVERY => {
                *held += 1;
                None
            }
            Some((at, held)) => {
                *at = now;
                Some(std::mem::take(held))
            }
            None => {
                map.insert(key, (now, 0));
                Some(0)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

#[cfg(test)]
#[path = "line_rate_tests.rs"]
mod tests;
