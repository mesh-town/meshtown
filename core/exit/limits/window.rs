//! The cap on unacknowledged bytes per exit stream toward the client.
//!
//! What it does: turns the `exit.stream_window_kib` setting and the memory tier default into a
//! byte count, kept in atomics so it can change while the node runs.
//! What it doesn't do: it doesn't touch open streams — each keeps the window it started with.

use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;

/// Smallest non-zero window, KiB: below it one stream gets too slow: about window ÷ round-trip time.
const STREAM_WINDOW_FLOOR_KIB: u64 = 64;

/// The window in bytes. `setting_kib` 0 takes `tier_kib`; any other value is at least
/// `STREAM_WINDOW_FLOOR_KIB`. A result of 0 means no window.
fn window_bytes(setting_kib: u64, tier_kib: u64) -> usize {
    let kib = match setting_kib {
        0 => tier_kib,
        n => n.max(STREAM_WINDOW_FLOOR_KIB),
    };
    usize::try_from(kib.saturating_mul(1024)).unwrap_or(usize::MAX)
}

/// The current window: the setting from the config file and the default of the memory tier.
#[derive(Default)]
pub(super) struct StreamWindow {
    setting_kib: AtomicU64,
    tier_kib: AtomicU64,
}

impl StreamWindow {
    /// Called once at process start.
    pub(super) fn configure(&self, setting_kib: u64, tier_kib: u64) {
        self.tier_kib.store(tier_kib, Ordering::Relaxed);
        self.set(setting_kib);
    }

    /// Changes the setting on the fly; the tier default stays.
    pub(super) fn set(&self, setting_kib: u64) {
        self.setting_kib.store(setting_kib, Ordering::Relaxed);
    }

    /// The window in bytes; 0 means no window.
    pub(super) fn bytes(&self) -> usize {
        window_bytes(
            self.setting_kib.load(Ordering::Relaxed),
            self.tier_kib.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
