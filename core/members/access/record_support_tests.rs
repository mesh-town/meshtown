//! Shared fixtures of the record tests: hand-assembled value bytes, sample keys and entries.

use super::*;
use crate::transport::net::{PeerId, ReplicaEntry};
use bytes::Bytes;

/// Value laid out by hand, field by field (not taken from `encode`): `Admit` of the ed25519 base
/// point via `Invite`, made at 1_792_400_000, note "laptop".
pub(in crate::members::access) const GOLDEN_ADMIT_V1: [u8; 47] = [
    0x01, // version
    0x01, // kind: Admit
    // subject: 32 bytes, the base point
    0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x01, // via: Invite
    0x80, 0xb5, 0xd7, 0xd6, 0x06, // made_unix 1_792_400_000, LEB128
    0x06, 0x6c, 0x61, 0x70, 0x74, 0x6f, 0x70, // note: length 6, "laptop"
];

/// The same layout as the admit golden, for an `Exclude` carrying the moment mark: kind 2, via 3 (`Witness`).
pub(in crate::members::access) const GOLDEN_EXCLUDE_AT_MOMENT_V1: [u8; 47] = [
    0x01, // version
    0x02, // kind: Exclude
    // subject: 32 bytes, the base point
    0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x03, // via: Witness, the moment mark
    0x80, 0xb5, 0xd7, 0xd6, 0x06, // made_unix 1_792_400_000, LEB128
    0x06, 0x6c, 0x61, 0x70, 0x74, 0x6f, 0x70, // note: length 6, "laptop"
];

/// The value of the golden test of the moment mark, for the given subject.
pub(in crate::members::access) fn exclude_at_moment_of(subject: PeerId) -> AclValue {
    AclValue {
        kind: AclKind::Exclude,
        subject,
        via: VOTE_AT_MOMENT,
        made_unix: 1_792_400_000,
        note: "laptop".to_string(),
    }
}

/// The made_unix of `Raw` values: the same 1_792_400_000 in LEB128.
pub(in crate::members::access) const MADE_UNIX_LEB128: [u8; 5] = [0x80, 0xb5, 0xd7, 0xd6, 0x06];

pub(in crate::members::access) fn base_bytes() -> [u8; 32] {
    let mut bytes = [0x66; 32];
    bytes[0] = 0x58;
    bytes
}

/// The ed25519 base point: surely on the curve.
pub(in crate::members::access) fn base() -> PeerId {
    PeerId::from_bytes(&base_bytes()).unwrap()
}

/// The first `[i; 32]` that is not a point of the curve.
pub(in crate::members::access) fn off_curve() -> [u8; 32] {
    let bytes = (0..=255u8)
        .map(|i| [i; 32])
        .find(|b| PeerId::from_bytes(b).is_err());
    bytes.expect("some [i; 32] must be off the curve")
}

/// The value of the golden test, for the given subject.
pub(in crate::members::access) fn admit_of(subject: PeerId) -> AclValue {
    AclValue {
        kind: AclKind::Admit,
        subject,
        via: Via::Invite,
        made_unix: 1_792_400_000,
        note: "laptop".to_string(),
    }
}

pub(in crate::members::access) fn entry(
    author: PeerId,
    key: Vec<u8>,
    value: Vec<u8>,
    ts_micros: u64,
) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key,
        value: Bytes::from(value),
        ts_micros,
    }
}

/// All kinds times all vias times the given notes.
pub(in crate::members::access) fn combinations(notes: &[String]) -> Vec<(AclKind, Via, String)> {
    let kinds = [
        AclKind::Admit,
        AclKind::Exclude,
        AclKind::Block,
        AclKind::Boot,
    ];
    let vias = [Via::None, Via::Invite, Via::Migration, Via::Witness];
    let mut out = vec![];
    for kind in kinds {
        for via in vias {
            out.extend(notes.iter().map(|n| (kind, via, n.clone())));
        }
    }
    out
}

/// Bytes of a version 1 value assembled by hand; every field can be broken on its own.
pub(in crate::members::access) struct Raw {
    pub(in crate::members::access) version: u8,
    pub(in crate::members::access) kind: u8,
    pub(in crate::members::access) subject: [u8; 32],
    pub(in crate::members::access) via: u8,
    pub(in crate::members::access) note: Vec<u8>,
}

impl Default for Raw {
    fn default() -> Raw {
        Raw {
            version: 1,
            kind: 1,
            subject: base_bytes(),
            via: 1,
            note: b"laptop".to_vec(),
        }
    }
}

pub(in crate::members::access) fn raw(r: Raw) -> Vec<u8> {
    let mut out = vec![r.version, r.kind];
    out.extend(r.subject);
    out.push(r.via);
    out.extend(MADE_UNIX_LEB128);
    out.extend(varint(r.note.len() as u64));
    out.extend(r.note);
    out
}

/// LEB128, as postcard writes lengths.
pub(in crate::members::access) fn varint(mut n: u64) -> Vec<u8> {
    let mut out = vec![];
    while n >= 0x80 {
        out.push((n & 0x7F) as u8 | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
    out
}
