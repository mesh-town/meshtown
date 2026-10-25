//! Merging of hints from several discovery sources: keeps live hints per node and source.
//! Rank: `"manual"` first, then the `order` sequence, the rest by name.

use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::discovery::Discovery;
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// Lifetime of a hint (like `ENTRY_TTL_SECS`).
pub const HINT_TTL: Duration = Duration::from_secs(900);
const MAX_HINTS_PER_SOURCE: usize = 16;
pub(crate) const MANUAL_SOURCE: &str = "manual";

/// Timeouts; a separate type so that tests can speed up time (the project's `tokio` has no `time::pause`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct DiscoveryTiming {
    pub ttl: Duration,
    pub publish_timeout: Duration,
}

impl DiscoveryTiming {
    pub(crate) const DEFAULT: DiscoveryTiming = DiscoveryTiming {
        ttl: HINT_TTL,
        publish_timeout: Duration::from_secs(30),
    };
}

struct SeenHint {
    hint: AddrHint,
    at: Instant,
}

type BySource = HashMap<&'static str, Vec<SeenHint>>;

pub struct DiscoveryMux {
    order: Vec<&'static str>,
    timing: DiscoveryTiming,
    sources: Vec<Arc<dyn Discovery>>,
    state: Mutex<HashMap<PeerId, BySource>>,
    news: broadcast::Sender<PeerAddr>,
    found_tap: Option<mpsc::Sender<PeerAddr>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    started: AtomicBool,
}

impl Drop for DiscoveryMux {
    fn drop(&mut self) {
        let tasks = self.tasks.get_mut().unwrap_or_else(PoisonError::into_inner);
        tasks.iter().for_each(|h| h.abort());
    }
}

impl DiscoveryMux {
    pub fn new(order: Vec<&'static str>) -> Self {
        Self::with_timing(order, DiscoveryTiming::DEFAULT)
    }

    pub(crate) fn with_timing(order: Vec<&'static str>, timing: DiscoveryTiming) -> Self {
        DiscoveryMux {
            order,
            timing,
            sources: Vec::new(),
            state: Mutex::new(HashMap::new()),
            news: broadcast::channel(256).0,
            found_tap: None,
            tasks: Mutex::new(Vec::new()),
            started: AtomicBool::new(false),
        }
    }

    pub fn add(&mut self, src: Arc<dyn Discovery>) {
        self.sources.push(src);
    }

    /// Tap of every source message (including repeats and addresses without hints);
    /// only before `start`, a repeat replaces the previous one.
    pub(crate) fn set_found_tap(&mut self, tx: mpsc::Sender<PeerAddr>) {
        self.found_tap = Some(tx);
    }

    /// Starts the sources and the cleaner; a repeated call does nothing.
    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut handles: Vec<_> = self
            .sources
            .iter()
            .flat_map(|s| self.spawn_source(s.clone()))
            .collect();
        handles.push(self.spawn_sweeper());
        lock(&self.tasks).extend(handles);
    }

    /// The source's `run_source` task and the `rx -> note` pump; the pump holds a `Weak`, there is no `Arc` cycle.
    fn spawn_source(self: &Arc<Self>, src: Arc<dyn Discovery>) -> [JoinHandle<()>; 2] {
        let (tx, rx) = mpsc::channel(64);
        let name = src.name();
        let weak: Weak<Self> = Arc::downgrade(self);
        let tap = self.found_tap.clone();
        let run = crate::transport::net::task_label::spawn(src.run_source(tx));
        let pump =
            crate::transport::net::task_label::spawn(Self::forward_reports(weak, name, tap, rx));
        [run, pump]
    }

    /// The `rx -> note` pump of one source; it ends when the mux is gone, the source closes or the tap is dropped.
    async fn forward_reports(
        weak: Weak<Self>,
        name: &'static str,
        tap: Option<mpsc::Sender<PeerAddr>>,
        mut rx: mpsc::Receiver<PeerAddr>,
    ) {
        while let Some(addr) = rx.recv().await {
            let Some(mux) = weak.upgrade() else { return };
            mux.note(name, addr.clone());
            drop(mux);
            if !Self::tap_send(&tap, addr).await {
                return;
            }
        }
    }

    /// Passes an address to the tap; `false` means the tap's receiver was dropped.
    async fn tap_send(tap: &Option<mpsc::Sender<PeerAddr>>, addr: PeerAddr) -> bool {
        let Some(tx) = tap else { return true };
        tx.send(addr).await.is_ok()
    }

    /// The broadcast of the `lookup` result arrives only with a new hint.
    pub fn note(&self, source: &'static str, addr: PeerAddr) {
        if addr.hints.is_empty() {
            return;
        }
        let merged = self.store_hint(source, &addr);
        if let Some(merged) = merged {
            let _ = self.news.send(merged);
        }
    }

    /// Records the hints of one report; returns the merged address if a new hint appeared.
    fn store_hint(&self, source: &'static str, addr: &PeerAddr) -> Option<PeerAddr> {
        let now = Instant::now();
        let mut state = lock(&self.state);
        let by_src = state.entry(addr.id).or_default();
        let list = by_src.entry(source).or_default();
        let fresh = Self::absorb(list, &addr.hints, now, self.timing.ttl);
        fresh.then(|| self.living(by_src, addr.id, now)).flatten()
    }

    /// Rule of `note`: new hints first, then the earlier live ones, at most 16. `true` means there was a new one.
    fn absorb(list: &mut Vec<SeenHint>, hints: &[AddrHint], now: Instant, ttl: Duration) -> bool {
        let live = |s: &SeenHint| now.saturating_duration_since(s.at) < ttl;
        let fresh = hints
            .iter()
            .any(|h| !list.iter().any(|s| live(s) && &s.hint == h));
        let mut next: Vec<SeenHint> = Vec::new();
        for hint in hints {
            if !next.iter().any(|s| &s.hint == hint) {
                next.push(SeenHint {
                    hint: hint.clone(),
                    at: now,
                });
            }
        }
        let old = std::mem::take(list);
        let kept = old
            .into_iter()
            .filter(|s| live(s) && !hints.contains(&s.hint));
        next.extend(kept);
        next.truncate(MAX_HINTS_PER_SOURCE);
        *list = next;
        fresh
    }

    pub fn lookup(&self, id: &PeerId) -> Option<PeerAddr> {
        let mut state = lock(&self.state);
        let by_src = state.get_mut(id)?;
        let found = self.living(by_src, *id, Instant::now());
        if found.is_none() {
            state.remove(id);
        }
        found
    }

    /// Rule of `lookup`: drops the dead, merges the live by source rank.
    fn living(&self, by_src: &mut BySource, id: PeerId, now: Instant) -> Option<PeerAddr> {
        let ttl = self.timing.ttl;
        for list in by_src.values_mut() {
            list.retain(|s| now.saturating_duration_since(s.at) < ttl);
        }
        by_src.retain(|_, list| !list.is_empty());
        let mut names: Vec<&'static str> = by_src.keys().copied().collect();
        names.sort_by_key(|n| self.rank(n));
        let mut merged = PeerAddr::new(id);
        for name in names {
            let hints = by_src[name].iter().map(|s| s.hint.clone()).collect();
            merged.merge(&PeerAddr { id, hints });
        }
        (!merged.hints.is_empty()).then_some(merged)
    }

    fn rank(&self, name: &'static str) -> (usize, &'static str) {
        if name == MANUAL_SOURCE {
            return (0, "");
        }
        let pos = self.order.iter().position(|n| *n == name);
        (1 + pos.unwrap_or(self.order.len()), name)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PeerAddr> {
        self.news.subscribe()
    }

    /// Added sources by rank; `"manual"` is not a source.
    pub fn source_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.sources.iter().map(|s| s.name()).collect();
        names.sort_by_key(|n| self.rank(n));
        names
    }

    /// Stops the source tasks; the data stays, a repeated call returns at once.
    /// A dropped mux aborts the tasks itself, without waiting.
    pub fn stop(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let handles = std::mem::take(&mut *lock(&self.tasks));
            handles.iter().for_each(|h| h.abort());
            for handle in handles {
                let _ = handle.await;
            }
        })
    }

    /// Nodes with at least one live hint, except `except`.
    pub(crate) fn known_peer_count(&self, except: &PeerId) -> usize {
        let now = Instant::now();
        let ttl = self.timing.ttl;
        let alive = |s: &SeenHint| now.saturating_duration_since(s.at) < ttl;
        let state = lock(&self.state);
        let has_live = |by_src: &BySource| by_src.values().flatten().any(alive);
        let known = state
            .iter()
            .filter(|(id, by_src)| *id != except && has_live(by_src));
        known.count()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

mod publish;

#[path = "discovery_mux_sweep.rs"]
mod sweep;

#[cfg(test)]
#[path = "discovery_mux_tests.rs"]
mod tests;
