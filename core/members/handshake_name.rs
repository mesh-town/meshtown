//! The node name that rides as a tail of the handshake: building and parsing the frame.
//!
//! What it does: packs a name into "2 length bytes + UTF-8" and reads it back, treating anything odd as no name.
//! What it doesn't do: no handshake exchange itself (see `handshake`).

use crate::members::words::{trim_and_validate_node_name, MAX_NAME_BYTES};

/// Name frame: an empty name or one longer than `MAX_NAME_BYTES` means "no name" (`[0, 0]`); otherwise the length
/// as `u16` big-endian (2 bytes) and the bytes themselves. It neither trims nor validates the name — the node's own name has already
/// passed validation (the default or `trim_and_validate_node_name`).
pub fn name_frame(name: &str) -> Vec<u8> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_NAME_BYTES {
        return vec![0, 0];
    }
    let mut out = Vec::with_capacity(2 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

/// Parses the handshake tail into the peer's name. Returns `None` on anything odd (shorter than 2 bytes, length 0 or
/// above `MAX_NAME_BYTES`, fewer bytes than promised, not UTF-8, rejected by `trim_and_validate_node_name`) —
/// the handshake still succeeds, the peer is simply treated as nameless. Bytes after the frame are not
/// looked at at all (room for future fields).
pub fn parse_name_tail(tail: &[u8]) -> Option<String> {
    let len = u16::from_be_bytes(tail.get(0..2)?.try_into().unwrap()) as usize;
    if len == 0 || len > MAX_NAME_BYTES {
        return None;
    }
    let name_bytes = tail.get(2..2 + len)?;
    let s = std::str::from_utf8(name_bytes).ok()?;
    trim_and_validate_node_name(s).ok().map(str::to_string)
}
