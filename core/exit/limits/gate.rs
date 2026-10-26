//! The counter of live exit streams: a cap, a slot guard and the reasons to refuse.

use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use super::line_rate::LineRate;
use super::rss::RssGate;
use crate::node::machine_memory::MemoryDefaults;

/// Why a new stream is refused.
#[derive(Debug)]
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
    lines: LineRate,
}

/// Live exit streams of the process and the rules for admitting a new one.
#[derive(Clone)]
pub struct ExitLimits(Arc<Inner>);

/// A held place among the live streams; dropping it frees the place.
#[derive(Debug)]
pub(crate) struct StreamSlot(ExitLimits);

impl std::fmt::Debug for ExitLimits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExitLimits")
            .field("live", &self.0.live.load(Ordering::Relaxed))
            .field("limit", &self.0.limit.load(Ordering::Relaxed))
            .finish()
    }
}

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
            lines: LineRate::default(),
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
    pub(crate) fn configure(&self, setting: u64, auto: u64, rss_ceiling_kib: u64) {
        self.0.auto.store(auto, Ordering::Relaxed);
        self.0.rss.set_ceiling(rss_ceiling_kib);
        self.set_max_streams(setting);
    }

    /// Changes the cap on the fly; open streams are not touched.
    pub(crate) fn set_max_streams(&self, setting: u64) {
        let limit = match setting {
            0 => self.0.auto.load(Ordering::Relaxed),
            n => n,
        };
        self.0.limit.store(limit, Ordering::Relaxed);
    }

    pub(crate) fn max_streams(&self) -> u64 {
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
        self.0.lines.due(now)
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
