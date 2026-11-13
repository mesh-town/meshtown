//! The exit's overload record in the network's shared document: the key, the parsing and the reading of others'.
//!
//! What it does: defines the value format (`<reason>[ <tail>]`, ASCII, at most 64 bytes) and reads the records
//! of all authors under the key `node/exit-load/`.
//! What it doesn't do: it doesn't write (that is `announce`) and doesn't touch the other exit keys.

use std::collections::HashMap;

use anyhow::Result;
use bytes::Bytes;

use super::verdict::Reason;
use crate::transport::net::{live, NetError, PeerId, Replica};

/// Key of the record in the shared document; the author is the exit itself.
pub(crate) const KEY: &[u8] = b"node/exit-load/";
/// The whole value, in bytes, at most.
const MAX_VALUE: usize = 64;
/// The first word, in bytes, at most.
const MAX_WORD: usize = 16;

/// Why an exit says it is overloaded, as a reader understands it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    /// A reason this version knows.
    Known(Reason),
    /// A word from a newer version.
    Other,
}

/// Another exit's record: the cause and the time it was written (microseconds, the author's clock).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExitLoad {
    pub(crate) cause: Cause,
    pub(crate) ts_micros: u64,
}

/// The value to write for a reason: just the word, no tail.
pub(crate) fn encode(reason: Reason) -> Bytes {
    Bytes::from_static(reason.as_str().as_bytes())
}

/// Reads a value: `Some` means "overloaded"; unreadable bytes mean `None`. The tail is ignored.
pub(crate) fn parse(value: &[u8]) -> Option<Cause> {
    if value.is_empty() || value.len() > MAX_VALUE || !value.is_ascii() {
        return None;
    }
    let word = value.split(|b| *b == b' ').next().unwrap_or_default();
    if word.is_empty() || word.len() > MAX_WORD || !word.iter().all(is_word_byte) {
        return None;
    }
    Some(cause_of(word))
}

fn is_word_byte(b: &u8) -> bool {
    b.is_ascii_lowercase() || *b == b'-'
}

fn cause_of(word: &[u8]) -> Cause {
    match word {
        b"limit" => Cause::Known(Reason::Limit),
        b"share" => Cause::Known(Reason::Share),
        b"memory" => Cause::Known(Reason::Memory),
        _ => Cause::Other,
    }
}

/// Overload records of the authors that have a live, readable one; tombstones and garbage are absent.
pub(crate) async fn read_exit_loads(r: &dyn Replica) -> Result<HashMap<PeerId, ExitLoad>> {
    let entries = r.scan(KEY).await.map_err(NetError::into_anyhow)?;
    Ok(live(entries)
        .into_iter()
        .filter(|e| e.key == KEY)
        .filter_map(|e| {
            let cause = parse(&e.value)?;
            Some((
                e.author,
                ExitLoad {
                    cause,
                    ts_micros: e.ts_micros,
                },
            ))
        })
        .collect())
}

#[cfg(test)]
#[path = "board_tests.rs"]
mod tests;
