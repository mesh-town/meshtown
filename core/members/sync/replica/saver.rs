//! Saving the replica to disk: at once (`save_now`) or after a second of silence (`save_debounced`); all saves go one at a time.

use super::entry::WireEntry;
use super::file::{self, Bak, ImportMark};
use super::state::State;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::replica::ReplicaId;
use portable_atomic::AtomicU64;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Silence before a deferred save.
const QUIET_BEFORE_SAVE: Duration = Duration::from_secs(1);

/// What to do with `.bak`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BakPolicy {
    /// `"domains"`: after each save the same bytes go to `.bak`.
    Mirror,
    /// `"net"`: the first save copies the previous file to `.bak` if it was read intact.
    FirstSave { main_ok: bool },
}

struct SaverState {
    path: Option<PathBuf>,
    id: ReplicaId,
    mark: Option<ImportMark>,
    policy: BakPolicy,
    /// The lock of all saves; inside is "the first save has already happened".
    lock: tokio::sync::Mutex<bool>,
    /// The generation of the deferred save: a new entry cancels the previous wait.
    pending: AtomicU64,
}

#[derive(Clone)]
pub(crate) struct Saver(Arc<SaverState>);

impl Saver {
    pub(crate) fn new(
        path: Option<PathBuf>,
        id: ReplicaId,
        mark: Option<ImportMark>,
        policy: BakPolicy,
    ) -> Saver {
        let lock = tokio::sync::Mutex::new(false);
        Saver(Arc::new(SaverState {
            path,
            id,
            mark,
            policy,
            lock,
            pending: AtomicU64::new(0),
        }))
    }

    /// Writes a state snapshot and returns when the file is already on disk. Without a path (memory) — nothing.
    pub(crate) async fn save_now(&self, state: &Mutex<State>) -> NetResult<()> {
        self.0.save(state).await
    }

    /// Writes a snapshot after a second without new calls; an error is a log line.
    pub(crate) fn save_debounced(&self, state: Arc<Mutex<State>>) {
        if self.0.path.is_none() {
            return;
        }
        let generation = self.0.pending.fetch_add(1, Ordering::SeqCst) + 1;
        let inner = self.0.clone();
        tokio::spawn(async move {
            tokio::time::sleep(QUIET_BEFORE_SAVE).await;
            if inner.pending.load(Ordering::SeqCst) != generation {
                return;
            }
            if let Err(e) = inner.save(&state).await {
                tracing::warn!("[sync] replica {} not saved: {e}", inner.id.fmt_short());
            }
        });
    }
}

impl SaverState {
    fn bak_for(&self, first_done: bool) -> Bak {
        match self.policy {
            BakPolicy::Mirror => Bak::Mirror,
            BakPolicy::FirstSave { main_ok: true } if !first_done => Bak::CopyPrevious,
            BakPolicy::FirstSave { .. } => Bak::Keep,
        }
    }

    async fn save(&self, state: &Mutex<State>) -> NetResult<()> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        let mut first_done = self.lock.lock().await;
        let entries: Vec<WireEntry> = state.lock().unwrap_or_else(|e| e.into_inner()).entries();
        let bytes = file::encode(&self.id, self.mark.as_ref(), &entries);
        let bak = self.bak_for(*first_done);
        let written = tokio::task::spawn_blocking(move || file::save(&path, &bytes, bak)).await;
        written
            .map_err(|e| NetError::Io(std::io::Error::other(e.to_string())))?
            .map_err(NetError::Io)?;
        *first_done = true;
        Ok(())
    }
}
