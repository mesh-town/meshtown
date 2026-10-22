//! A narrow DHT record store trait: the record key, the record, and reading/writing the value.

use crate::net::{BoxFuture, NetResult};

/// Record key: the signing seed (the key pair is built on the transport side).
#[derive(Clone, PartialEq, Eq)]
pub struct RecordKey([u8; 32]);

impl RecordKey {
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(seed)
    }

    pub fn seed(&self) -> [u8; 32] {
        self.0
    }
}

impl std::fmt::Debug for RecordKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecordKey(..)")
    }
}

/// A read record: the sequence number and the value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub seq: u64,
    pub value: Vec<u8>,
}

/// DHT record store. A `put` with a `seq` not greater than the stored one may silently not write.
pub trait RecordStore: Send + Sync {
    fn get(&self, key: &RecordKey) -> BoxFuture<'_, NetResult<Option<Record>>>;
    fn put(&self, key: &RecordKey, value: Vec<u8>, seq: u64) -> BoxFuture<'_, NetResult<()>>;
}
