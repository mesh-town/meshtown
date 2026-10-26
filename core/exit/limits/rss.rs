//! Process memory check for the exit: a cached reading compared with a ceiling.

use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long one reading of the process memory is reused.
const RSS_CACHE: Duration = Duration::from_secs(1);

/// Headroom kept below the warning level, in KiB: room for one more heavy stream.
const HEADROOM_KIB: u64 = 1024;

/// Refuses new streams when the process memory reaches the ceiling.
pub(crate) struct RssGate {
    ceiling_kib: AtomicU64,
    read: fn() -> u64,
    cached: Mutex<Option<(Instant, u64)>>,
}

impl RssGate {
    pub(crate) fn new(ceiling_kib: u64, read: fn() -> u64) -> Self {
        Self {
            ceiling_kib: AtomicU64::new(ceiling_kib),
            read,
            cached: Mutex::new(None),
        }
    }

    /// The production gate: reads the memory of this process; no ceiling until configured.
    pub(crate) fn process() -> Self {
        Self::new(0, process_rss_kib)
    }

    pub(crate) fn set_ceiling(&self, ceiling_kib: u64) {
        self.ceiling_kib.store(ceiling_kib, Ordering::Relaxed);
    }

    /// `(rss, ceiling)` in KiB when the memory is at or above the ceiling.
    /// No ceiling or no reading means "not over"; the ceiling 0 never reads the memory.
    pub(crate) fn over(&self, now: Instant) -> Option<(u64, u64)> {
        let ceiling = self.ceiling_kib.load(Ordering::Relaxed);
        if ceiling == 0 {
            return None;
        }
        let rss = self.reading(now);
        (rss != 0 && rss >= ceiling).then_some((rss, ceiling))
    }

    fn reading(&self, now: Instant) -> u64 {
        let mut cached = self.cached.lock().unwrap_or_else(|e| e.into_inner());
        match *cached {
            Some((at, value)) if now.saturating_duration_since(at) < RSS_CACHE => value,
            _ => {
                let value = (self.read)();
                *cached = Some((now, value));
                value
            }
        }
    }
}

fn process_rss_kib() -> u64 {
    crate::node::telemetry::rss_kib().0
}

/// Memory ceiling in KiB for the warning level in MiB: one MiB below it, at least one MiB.
/// The warning level 0 means "guard off" and gives 0.
pub(crate) fn rss_ceiling_kib(warn_mib: u64) -> u64 {
    if warn_mib == 0 {
        return 0;
    }
    (warn_mib.saturating_mul(1024).saturating_sub(HEADROOM_KIB)).max(HEADROOM_KIB)
}

#[cfg(test)]
#[path = "rss_tests.rs"]
mod tests;
