//! Replica: a shared store of "author, key, value" records, each author with its own key space.
//! Only data and a trait here; implementations live behind the sync seam (`sync_backend`).

use crate::net::error::NetResult;
use crate::net::id::PeerId;
use crate::net::stream::BoxFuture;
use bytes::Bytes;
use data_encoding::HEXLOWER;

/// Value limit of a record: `set_own` with a longer value answers `Err(Config)`.
pub const MAX_VALUE_BYTES: usize = 4096;

/// Replica record. An empty value is a tombstone (the record is deleted).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplicaEntry {
    pub author: PeerId,
    pub key: Vec<u8>,
    pub value: Bytes,
    /// Unix microseconds by the author's clock.
    pub ts_micros: u64,
}

impl ReplicaEntry {
    pub fn is_tombstone(&self) -> bool {
        self.value.is_empty()
    }
}

/// Opaque replica id: needed only for a log line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReplicaId(pub [u8; 32]);

impl ReplicaId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// First 10 hex digits, lowercase.
    pub fn fmt_short(&self) -> String {
        HEXLOWER.encode(&self.0)[..10].to_string()
    }
}

/// How many records were received and sent in one reconciliation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncCount {
    pub received: u64,
    pub sent: u64,
}

/// Replica event.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplicaEvent {
    /// An own record or tombstone landed.
    LocalChange,
    /// A foreign record, tombstone or content arrived.
    RemoteChange,
    /// Result of one reconciliation — for the log only.
    Synced {
        peer: PeerId,
        outcome: Result<SyncCount, String>,
    },
    /// A reconciliation peer came or left.
    Neighbor { peer: PeerId, up: bool },
}

pub trait Replica: Send + Sync {
    fn id(&self) -> ReplicaId;
    /// Id of the own node as the author of records.
    fn me(&self) -> PeerId;
    /// Writes an own record; a value longer than `MAX_VALUE_BYTES` gives `Err(Config)`.
    fn set_own(&self, key: &[u8], value: Bytes) -> BoxFuture<'_, NetResult<()>>;
    /// Puts a tombstone on the exact own key.
    fn del_own(&self, key: &[u8]) -> BoxFuture<'_, NetResult<()>>;
    fn get(&self, author: PeerId, key: &[u8]) -> BoxFuture<'_, NetResult<Option<ReplicaEntry>>>;
    /// All records of all authors with such a key prefix, tombstones included.
    fn scan(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>>;
    /// Last record for each key among the authors, tombstones included.
    fn latest_per_key(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>>;
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ReplicaEvent>;
    /// Turns on reconciliation with network members.
    fn start_sync(&self) -> BoxFuture<'_, NetResult<()>>;
    /// One-off reconciliation with a single node.
    fn sync_with(&self, peer: PeerId) -> BoxFuture<'_, NetResult<()>>;
    /// Writes the pending state to disk now (a replica that saves on a delay); by default there is nothing to do.
    fn flush(&self) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async { Ok(()) })
    }
}

/// Without tombstones, order preserved.
pub fn live(entries: Vec<ReplicaEntry>) -> Vec<ReplicaEntry> {
    entries.into_iter().filter(|e| !e.is_tombstone()).collect()
}

#[cfg(test)]
#[path = "replica_tests.rs"]
mod tests;
