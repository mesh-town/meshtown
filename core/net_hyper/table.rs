//! Session table: which session an incoming data stream belongs to.
//! The key includes the other side and whose number names the session, so a foreign peer with the same
//! number does not end up in somebody else's session.

use crate::net::id::PeerId;
use crate::net::stream::StreamPair;
use crate::net_hyper::wire::{REFUSE_SESSION_CLOSED, REFUSE_NO_SESSION};
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

/// Address of a session in the table: the other side, the number, and whether we named the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionKey {
    pub peer: PeerId,
    pub session: u64,
    pub numbered_by_us: bool,
}

/// Live sessions of the transport: where to hand streams that arrived with a `Stream` header.
#[derive(Default)]
pub struct Sessions {
    stream_queues: Mutex<HashMap<SessionKey, mpsc::Sender<StreamPair>>>,
}

impl Sessions {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_stream_queues(&self) -> MutexGuard<'_, HashMap<SessionKey, mpsc::Sender<StreamPair>>> {
        self.stream_queues.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn insert(&self, key: SessionKey, tx: mpsc::Sender<StreamPair>) {
        self.lock_stream_queues().insert(key, tx);
    }

    /// Takes a slot in the session queue before `Ack` is written: the stream is handed over only once the reply has gone out.
    /// `Err` — the refusal code; a closed queue is forgotten.
    pub fn reserve(&self, key: SessionKey) -> Result<mpsc::OwnedPermit<StreamPair>, u32> {
        let tx = self.lock_stream_queues().get(&key).cloned().ok_or(REFUSE_NO_SESSION)?;
        tx.try_reserve_owned().map_err(|e| {
            if matches!(e, TrySendError::Closed(_)) {
                self.remove(key);
            }
            REFUSE_SESSION_CLOSED
        })
    }

    pub fn remove(&self, key: SessionKey) {
        self.lock_stream_queues().remove(&key);
    }

    /// Forgets all sessions; their receivers see the end of the queue.
    pub fn close_all(&self) {
        self.lock_stream_queues().clear();
    }
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
