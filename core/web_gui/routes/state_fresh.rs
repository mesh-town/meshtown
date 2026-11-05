//! `/api/state` after an idle page: the node collects its snapshot only while the page reads it, so the first
//! read after a pause sees an old one. The route waits a little for the collection that read woke.

use std::time::Duration;

use super::super::SlotHandle;

/// A snapshot older than this is "old" (the node's collection pauses after as much idle time).
const STALE_AFTER_MS: u64 = 5_000;
/// How long the route waits for a fresh snapshot before it answers with what it has.
const WAIT_LIMIT: Duration = Duration::from_secs(2);
/// How often the age is asked again while waiting.
const POLL_EVERY: Duration = Duration::from_millis(100);

/// Reads the snapshot (which also wakes the collection); while it is old, asks again every 100 ms for at most 2 s.
pub(super) async fn wait_for_fresh_snapshot(slot: &SlotHandle) {
    if slot.node.snapshot().snapshot_age_ms <= STALE_AFTER_MS {
        return;
    }
    let deadline = tokio::time::Instant::now() + WAIT_LIMIT;
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(POLL_EVERY).await;
        if slot.node.snapshot().snapshot_age_ms <= STALE_AFTER_MS {
            return;
        }
    }
}
