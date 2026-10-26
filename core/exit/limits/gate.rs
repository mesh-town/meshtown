//! The counter of live exit streams: a cap, a slot guard and the reasons to refuse.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::rss::RssGate;
use crate::node::machine_memory::MemoryDefaults;

/// Refusals are written to the log at most this often.
const BUSY_LINE_EVERY: Duration = Duration::from_secs(10);

/// Why a new stream is refused.
pub(crate) enum Busy {
    Streams { live: u64, limit: u64 },
    Memory { rss_kib: u64, ceiling_kib: u64 },
}

impl Busy {
    pub(crate) fn line(&self) -> String {
        match self {
            Busy::Streams { live, limit } => {
                format!("busy: {live} streams open, limit {limit}")
            }
            Busy::Memory {
                rss_kib,
                ceiling_kib,
            } => format!(
                "busy: memory {} MiB, ceiling {} MiB",
                rss_kib / 1024,
                ceiling_kib / 1024
            ),
        }
    }
}

struct Inner {
    live: AtomicU64,
    limit: AtomicU64,
    auto: AtomicU64,
    rss: RssGate,
    refused: AtomicU64,
    last_line: Mutex<Option<Instant>>,
}

/// Live exit streams of the process and the rules for admitting a new one.
#[derive(Clone)]
pub struct ExitLimits(Arc<Inner>);

/// A held place among the live streams; dropping it frees the place.
pub(crate) struct StreamSlot(ExitLimits);

impl Drop for StreamSlot {
    fn drop(&mut self) {
        self.0 .0.live.fetch_sub(1, Ordering::Relaxed);
    }
}

impl ExitLimits {
    fn build(limit: u64, rss: RssGate) -> Self {
        Self(Arc::new(Inner {
            live: AtomicU64::new(0),
            limit: AtomicU64::new(limit),
            auto: AtomicU64::new(limit),
            rss,
            refused: AtomicU64::new(0),
            last_line: Mutex::new(None),
        }))
    }

    /// The counter shared by every network of the process.
    pub fn process() -> ExitLimits {
        static PROCESS: OnceLock<ExitLimits> = OnceLock::new();
        PROCESS
            .get_or_init(|| Self::build(MemoryDefaults::LARGE.max_streams, RssGate::process()))
            .clone()
    }

    /// Called once at process start. `setting` 0 means the automatic cap `auto`.
    pub fn configure(&self, setting: u64, auto: u64, rss_ceiling_kib: u64) {
        self.0.auto.store(auto, Ordering::Relaxed);
        self.0.rss.set_ceiling(rss_ceiling_kib);
        self.set_max_streams(setting);
    }

    /// Changes the cap on the fly; open streams are not touched.
    pub fn set_max_streams(&self, setting: u64) {
        let limit = match setting {
            0 => self.0.auto.load(Ordering::Relaxed),
            n => n,
        };
        self.0.limit.store(limit, Ordering::Relaxed);
    }

    pub fn max_streams(&self) -> u64 {
        self.0.limit.load(Ordering::Relaxed)
    }

    /// Takes a place for a new stream: the cap first, then the process memory.
    pub(crate) fn try_admit(&self) -> Result<StreamSlot, Busy> {
        let limit = self.max_streams();
        self.take_place(limit)?;
        let slot = StreamSlot(self.clone());
        match self.0.rss.over(Instant::now()) {
            Some((rss_kib, ceiling_kib)) => Err(Busy::Memory {
                rss_kib,
                ceiling_kib,
            }),
            None => Ok(slot),
        }
    }

    fn take_place(&self, limit: u64) -> Result<(), Busy> {
        let taken = self
            .0
            .live
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                (n < limit).then_some(n + 1)
            });
        taken
            .map(|_| ())
            .map_err(|live| Busy::Streams { live, limit })
    }

    /// `Some(refusals held back since the last line)` when a log line is due, else `None`
    /// (the refusal is counted for the next line).
    pub(crate) fn busy_line_due(&self, now: Instant) -> Option<u64> {
        let mut last = self.0.last_line.lock().unwrap_or_else(|e| e.into_inner());
        let due = last.is_none_or(|at| now.saturating_duration_since(at) >= BUSY_LINE_EVERY);
        if !due {
            self.0.refused.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        *last = Some(now);
        Some(self.0.refused.swap(0, Ordering::Relaxed))
    }
}

#[cfg(test)]
impl ExitLimits {
    /// A counter with a fixed cap and no memory guard.
    pub(crate) fn fixed(max_streams: u64) -> Self {
        Self::build(max_streams, RssGate::new(0, || 0))
    }

    pub(crate) fn live(&self) -> u64 {
        self.0.live.load(Ordering::Relaxed)
    }

    pub(crate) fn unlimited() -> Self {
        Self::fixed(u64::MAX)
    }

    pub(crate) fn with_rss(max_streams: u64, ceiling_kib: u64, read: fn() -> u64) -> Self {
        Self::build(max_streams, RssGate::new(ceiling_kib, read))
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
