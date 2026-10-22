//! A signed replica entry: the wire form, the bytes to sign, verification and the win order on merge.

use crate::net::id::{NodeSecret, PeerId};
use crate::net::replica::{ReplicaEntry, ReplicaId, MAX_VALUE_BYTES};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

/// The entry key limit on receipt (the longest consumer is `chk/<domain ≤253>/`).
pub(crate) const MAX_KEY_BYTES: usize = 512;

const SIGN_CONTEXT: &[u8] = b"meshtown replica entry v1\0";

/// An entry on the wire and in the file. `sig` is two halves of an ed25519 signature (serde does not take arrays longer than 32).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WireEntry {
    pub author: [u8; 32],
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub ts_micros: u64,
    pub sig: [[u8; 32]; 2],
}

/// Why an entry was dropped on receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reject {
    BadAuthor,
    KeyTooLong,
    ValueTooLong,
    BadSignature,
}

/// The bytes to sign: context, replica, author, `ts_micros` (u64 BE), key length (u32 BE), key, value.
pub(crate) fn signed_bytes(
    id: &ReplicaId,
    author: &PeerId,
    key: &[u8],
    value: &[u8],
    ts_micros: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(SIGN_CONTEXT.len() + 80 + key.len() + value.len());
    out.extend_from_slice(SIGN_CONTEXT);
    out.extend_from_slice(id.as_bytes());
    out.extend_from_slice(author.as_bytes());
    out.extend_from_slice(&ts_micros.to_be_bytes());
    out.extend_from_slice(&(key.len() as u32).to_be_bytes());
    out.extend_from_slice(key);
    out.extend_from_slice(value);
    out
}

fn split_sig(sig: [u8; 64]) -> [[u8; 32]; 2] {
    let (a, b) = sig.split_at(32);
    [a.try_into().expect("32"), b.try_into().expect("32")]
}

fn join_sig(sig: &[[u8; 32]; 2]) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&sig[0]);
    out[32..].copy_from_slice(&sig[1]);
    out
}

/// Signs an entry with the author node's key.
pub(crate) fn seal(
    secret: &NodeSecret,
    id: &ReplicaId,
    key: &[u8],
    value: &[u8],
    ts_micros: u64,
) -> WireEntry {
    let author = secret.peer_id();
    let sig = secret.sign(&signed_bytes(id, &author, key, value, ts_micros));
    WireEntry {
        author: *author.as_bytes(),
        key: key.to_vec(),
        value: value.to_vec(),
        ts_micros,
        sig: split_sig(sig),
    }
}

/// Checks the limits, the author and the signature; returns the author.
pub(crate) fn check(id: &ReplicaId, w: &WireEntry) -> Result<PeerId, Reject> {
    if w.key.len() > MAX_KEY_BYTES {
        return Err(Reject::KeyTooLong);
    }
    if w.value.len() > MAX_VALUE_BYTES {
        return Err(Reject::ValueTooLong);
    }
    let author = PeerId::from_bytes(&w.author).map_err(|_| Reject::BadAuthor)?;
    let msg = signed_bytes(id, &author, &w.key, &w.value, w.ts_micros);
    let ok = author.verify(&msg, &join_sig(&w.sig));
    ok.then_some(author).ok_or(Reject::BadSignature)
}

pub(crate) fn to_entry(author: PeerId, w: &WireEntry) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key: w.key.clone(),
        value: Bytes::copy_from_slice(&w.value),
        ts_micros: w.ts_micros,
    }
}

/// Whether `new` wins over `old` of the same "(author, key)" pair: the larger `ts_micros`, on a tie the larger value.
pub(crate) fn wins(new: &WireEntry, old: &WireEntry) -> bool {
    (new.ts_micros, &new.value) > (old.ts_micros, &old.value)
}

#[cfg(test)]
#[path = "entry_tests.rs"]
mod tests;
