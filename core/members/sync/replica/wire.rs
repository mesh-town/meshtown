//! The reconciliation and broadcast wire of the replica: `REPLICA` stream frames, the broadcast body, slicing entries into batches.

use super::entry::WireEntry;
use super::state::WriterSum;
use crate::members::sync::flood::wire::MAX_BODY_BYTES;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::replica::ReplicaId;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind};
use std::time::Duration;

pub(crate) const PROTO_VERSION: u16 = 1;
/// The `Entries` batch limit in postcard, bytes.
pub(crate) const MAX_BATCH_BYTES: usize = 48 * 1024;
/// How many entries one reconciliation session accepts.
pub(crate) const MAX_SESSION_ENTRIES: usize = 50_000;
pub(crate) const SESSION_TIMEOUT: Duration = Duration::from_secs(30);

/// Headroom for the variant tag and the vector length in an `Entries` frame.
const BATCH_OVERHEAD: usize = 8;

/// A `REPLICA` stream frame after the handshake.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Frame {
    Summary {
        version: u16,
        writers: Vec<WriterSum>,
    },
    Entries(Vec<WireEntry>),
    Done {
        sent: u32,
    },
}

/// The body of a live broadcast through topic membership.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Push {
    pub version: u16,
    pub replica: [u8; 32],
    pub entries: Vec<WireEntry>,
}

fn invalid(what: &str) -> NetError {
    NetError::Io(Error::new(ErrorKind::InvalidData, what.to_string()))
}

fn size_of(w: &WireEntry) -> usize {
    postcard::to_stdvec(w)
        .map(|b| b.len())
        .unwrap_or(usize::MAX)
}

/// Slices entries into batches, each no more than `MAX_BATCH_BYTES` in postcard; the order is preserved.
pub(crate) fn batches(all: Vec<WireEntry>) -> Vec<Vec<WireEntry>> {
    let mut out: Vec<Vec<WireEntry>> = Vec::new();
    let mut used = BATCH_OVERHEAD;
    for w in all {
        let size = size_of(&w);
        let fits = out.last().is_some() && used + size <= MAX_BATCH_BYTES;
        if !fits {
            out.push(Vec::new());
            used = BATCH_OVERHEAD;
        }
        used += size;
        out.last_mut().expect("there is a batch").push(w);
    }
    out
}

/// The broadcast body; longer than `MAX_BODY_BYTES` is `Io(InvalidData)`.
pub(crate) fn encode_push(id: &ReplicaId, entries: Vec<WireEntry>) -> NetResult<Bytes> {
    let push = Push {
        version: PROTO_VERSION,
        replica: id.0,
        entries,
    };
    let body = postcard::to_stdvec(&push).map_err(|_| invalid("broadcast cannot be serialized"))?;
    if body.len() > MAX_BODY_BYTES {
        return Err(invalid("broadcast longer than the limit"));
    }
    Ok(Bytes::from(body))
}

pub(crate) fn decode_push(body: &[u8]) -> NetResult<Push> {
    postcard::from_bytes(body).map_err(|_| invalid("corrupt broadcast"))
}

#[cfg(test)]
#[path = "wire_tests.rs"]
mod tests;
