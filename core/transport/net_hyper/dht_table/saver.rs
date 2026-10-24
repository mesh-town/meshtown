//! Background writer of the DHT table: the first write soon after the start, then periodically,
//! and a last one on close. A write happens only when the node set changed or the file grows stale.

use super::file::{save, MAX_NODES};
use crate::node::output::say;
use crate::transport::net_mainline::unix_now;
use std::net::SocketAddrV4;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::{interval_at, timeout, Instant, MissedTickBehavior};
use townjump_dht::Dht;
use townjump_dht_peeroxide::PeeroxideDht;

/// Delay of the first write after the start.
pub(crate) const FIRST_SAVE_AFTER: Duration = Duration::from_secs(60);
/// Interval of the periodic writes.
pub(crate) const SAVE_EVERY: Duration = Duration::from_secs(600);
/// An unchanged table is rewritten after this many seconds so that the stamp does not go stale.
pub(crate) const REFRESH_AFTER: u64 = 24 * 3600;
/// Upper bound on the last write at close: a stuck DHT actor does not hold the shutdown.
pub(crate) const FINISH_LIMIT: Duration = Duration::from_secs(2);

/// The last table written by this node.
pub(crate) struct Written {
    sorted: Vec<SocketAddrV4>,
    at: u64,
}

impl Written {
    pub(crate) fn new(nodes: &[SocketAddrV4], at: u64) -> Written {
        let mut sorted = nodes.to_vec();
        sorted.sort();
        Written { sorted, at }
    }
}

/// Whether `nodes` must be written now: never an empty table; always the first one; then on a change or when stale.
pub(crate) fn should_write(last: Option<&Written>, nodes: &[SocketAddrV4], now: u64) -> bool {
    if nodes.is_empty() {
        return false;
    }
    let Some(last) = last else {
        return true;
    };
    let stale = now.saturating_sub(last.at) >= REFRESH_AFTER;
    stale || Written::new(nodes, now).sorted != last.sorted
}

/// State shared by the periodic task and the last write.
struct Shared {
    node: Arc<PeeroxideDht>,
    path: PathBuf,
    last: Mutex<Option<Written>>,
    /// Serialises file writes: an aborted task may still have a blocking write running.
    writing: Arc<Mutex<()>>,
    told: AtomicBool,
}

pub(crate) struct TableSaver {
    shared: Arc<Shared>,
    task: tokio::sync::Mutex<Option<JoinHandle<()>>>,
    abort: tokio::task::AbortHandle,
}

impl TableSaver {
    pub(crate) fn start(node: Arc<PeeroxideDht>, path: PathBuf) -> TableSaver {
        TableSaver::start_with(node, path, FIRST_SAVE_AFTER, SAVE_EVERY)
    }

    pub(crate) fn start_with(
        node: Arc<PeeroxideDht>,
        path: PathBuf,
        first: Duration,
        every: Duration,
    ) -> TableSaver {
        let shared = Arc::new(Shared {
            node,
            path,
            last: Mutex::new(None),
            writing: Arc::new(Mutex::new(())),
            told: AtomicBool::new(false),
        });
        let task = crate::transport::net::task_label::spawn(periodic(shared.clone(), first, every));
        let abort = task.abort_handle();
        TableSaver {
            shared,
            task: tokio::sync::Mutex::new(Some(task)),
            abort,
        }
    }

    /// Stops the periodic task, waits for it and writes the last time (bounded by `FINISH_LIMIT`).
    pub(crate) async fn finish(&self) {
        let Some(task) = self.task.lock().await.take() else {
            return;
        };
        task.abort();
        let _ = task.await;
        let _ = timeout(FINISH_LIMIT, write_table(&self.shared)).await;
    }
}

impl Drop for TableSaver {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

async fn periodic(shared: Arc<Shared>, first: Duration, every: Duration) {
    let mut ticks = interval_at(Instant::now() + first, every);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        write_table(&shared).await;
    }
}

/// An error or an unchanged table: nothing is written.
async fn write_table(shared: &Arc<Shared>) {
    let Ok(nodes) = shared.node.to_array(Some(MAX_NODES)).await else {
        return;
    };
    let now = unix_now();
    if wanted(shared, &nodes, now) {
        write_nodes(shared, nodes, now).await;
    }
}

fn wanted(shared: &Shared, nodes: &[SocketAddrV4], now: u64) -> bool {
    let last = shared.last.lock();
    let last = last.as_ref().ok().and_then(|guard| guard.as_ref());
    should_write(last, nodes, now)
}

async fn write_nodes(shared: &Arc<Shared>, nodes: Vec<SocketAddrV4>, now: u64) {
    let (path, writing) = (shared.path.clone(), shared.writing.clone());
    let copy = nodes.clone();
    let done = crate::transport::net::task_label::spawn_blocking(move || {
        let _guard = writing.lock();
        save(&path, &copy, now)
    })
    .await;
    match done {
        Ok(Ok(())) => remember(shared, &nodes, now),
        Ok(Err(e)) => tell_once(shared, &e),
        Err(_) => {}
    }
}

fn remember(shared: &Shared, nodes: &[SocketAddrV4], now: u64) {
    if let Ok(mut last) = shared.last.lock() {
        *last = Some(Written::new(nodes, now));
    }
}

fn tell_once(shared: &Shared, e: &std::io::Error) {
    if shared.told.swap(true, Ordering::Relaxed) {
        return;
    }
    let path = shared.path.display();
    say("hyper", format!("DHT table {path} not written: {e}"));
}

#[cfg(test)]
#[path = "saver_tests.rs"]
mod tests;
