//! Per-level event counters on top of `common/log_capture.rs`.
//! Plugged in as `#[path = "common/log_capture_counts.rs"] mod log_capture_counts;` next to `mod log_capture;`.

use tracing::Level;

use super::log_capture::{since, Mark};

/// How many events of level `level` have appeared since the mark.
pub(crate) fn count_since(mark: Mark, level: Level) -> usize {
    since(mark).iter().filter(|e| e.level == level).count()
}

/// How many `WARN`s since the mark.
pub(crate) fn warns_since(mark: Mark) -> usize {
    count_since(mark, Level::WARN)
}
