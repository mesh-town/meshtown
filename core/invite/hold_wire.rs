//! HOLD frames: the issuing node's request "save the invite entry" and the member's answer.
//!
//! What it does: assembles and parses the request frame (73 bytes, signed with the network key) and the answer
//! (34 bytes). The service is the same as for admission (`meshtown/invite/1`); the first byte of the frame is the kind.
//! What it doesn't do: doesn't go to the network and doesn't write to the document; the holder's write is `invite::server`.
//! Links: the server answers HOLD, the issuing node asks the holders; the codes are in `invite::wire`.

use super::wire::{RedeemCode, REPLY_LEN};
use crate::members::network_key::NetworkKey;

/// The HOLD frame kind: the first byte (for admission — 1).
pub const HOLD_FRAME_KIND: u8 = 2;
/// The request length: kind, secret hash (32), issue time (8), signature (32).
pub const HOLD_LEN: usize = 73;
/// The length of the signed part: everything except the signature.
const SIGNED_LEN: usize = 41;
/// The signing key context: domain separation from the network's other keys.
pub const CTX_INVITE_HOLD: &str = "meshtown invite hold v1";
/// How many seconds ahead the issue time is still accepted (otherwise the entry would live longer than a day).
pub const HOLD_SKEW_SECS: u64 = 600;

/// A parsed HOLD request: the secret hash (not the secret itself) and the issue time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoldReq {
    pub hash: [u8; 32],
    pub made_unix: u64,
}

/// The signature of the first `SIGNED_LEN` bytes with a key derived from the network key.
fn sign(key: &NetworkKey, signed: &[u8]) -> blake3::Hash {
    let mac_key = blake3::derive_key(CTX_INVITE_HOLD, &key.to_bytes());
    blake3::keyed_hash(&mac_key, signed)
}

/// The HOLD request frame: `[2][hash 32][made_unix LE 8][signature 32]`.
pub fn hold_frame(key: &NetworkKey, hash: &[u8; 32], made_unix: u64) -> [u8; HOLD_LEN] {
    let mut out = [0u8; HOLD_LEN];
    out[0] = HOLD_FRAME_KIND;
    out[1..33].copy_from_slice(hash);
    out[33..SIGNED_LEN].copy_from_slice(&made_unix.to_le_bytes());
    let mac = sign(key, &out[..SIGNED_LEN]);
    out[SIGNED_LEN..].copy_from_slice(mac.as_bytes());
    out
}

/// Parses a HOLD request; a foreign kind or signature gives `None` (the signature is compared in constant time).
pub fn parse_hold(key: &NetworkKey, b: &[u8; HOLD_LEN]) -> Option<HoldReq> {
    if b[0] != HOLD_FRAME_KIND {
        return None;
    }
    let mut mac = [0u8; 32];
    mac.copy_from_slice(&b[SIGNED_LEN..]);
    if sign(key, &b[..SIGNED_LEN]) != blake3::Hash::from_bytes(mac) {
        return None;
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&b[1..33]);
    let mut time = [0u8; 8];
    time.copy_from_slice(&b[33..SIGNED_LEN]);
    Some(HoldReq {
        hash,
        made_unix: u64::from_le_bytes(time),
    })
}

/// The HOLD reply frame: `[2][code][32 zeros]`.
pub fn hold_reply(code: RedeemCode) -> [u8; REPLY_LEN] {
    let mut out = [0u8; REPLY_LEN];
    out[0] = HOLD_FRAME_KIND;
    out[1] = code as u8;
    out
}

/// Parses a HOLD reply; a foreign kind, an unknown code and a nonzero tail give `None`.
pub fn parse_hold_reply(b: &[u8; REPLY_LEN]) -> Option<RedeemCode> {
    if b[0] != HOLD_FRAME_KIND || b[2..].iter().any(|x| *x != 0) {
        return None;
    }
    RedeemCode::from_u8(b[1])
}

#[cfg(test)]
#[path = "hold_wire_tests.rs"]
mod tests;
