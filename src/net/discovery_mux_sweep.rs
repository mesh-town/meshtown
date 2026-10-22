//! Cleanup of the discovery merge `state`: a periodic task and a single pass.

use super::{lock, DiscoveryMux};
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::Instant;

const MIN_PERIOD: Duration = Duration::from_millis(10);

impl DiscoveryMux {
    /// Drops dead hints, empty sources and nodes without sources; sends no news.
    pub(crate) fn sweep(&self) {
        let now = Instant::now();
        let ttl = self.timing.ttl;
        let mut state = lock(&self.state);
        for by_src in state.values_mut() {
            for list in by_src.values_mut() {
                list.retain(|s| now.saturating_duration_since(s.at) < ttl);
            }
            by_src.retain(|_, list| !list.is_empty());
        }
        state.retain(|_, by_src| !by_src.is_empty());
    }

    /// A task that calls `sweep` once per `ttl`; it holds a `Weak`, so a dropped mux is not kept alive by it.
    pub(super) fn spawn_sweeper(self: &Arc<Self>) -> JoinHandle<()> {
        let weak: Weak<Self> = Arc::downgrade(self);
        let period = self.timing.ttl.max(MIN_PERIOD);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(period).await;
                let Some(mux) = weak.upgrade() else { return };
                mux.sweep();
            }
        })
    }
}
