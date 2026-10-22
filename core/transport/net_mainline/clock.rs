//! Adapter clock: the current time in Unix seconds. Without the `mainline` crate — available with any feature set.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current time in seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
