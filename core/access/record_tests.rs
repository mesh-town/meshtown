//! Tests of the membership list record format (`record.rs`): value bytes, keys, parsing of
//! replica entries. The tally input builder is in `record_input_tests.rs`, writing and withdrawing
//! own records in `record_write_tests.rs`.

use super::*;
use crate::net::contract::pid;
use crate::net::{PeerId, ReplicaEntry};
use bytes::Bytes;
use data_encoding::HEXLOWER;

/// Value laid out by hand, field by field (not taken from `encode`): `Admit` of the ed25519 base
/// point via `Invite`, made at 1_792_400_000, note "laptop".
const GOLDEN_ADMIT_V1: [u8; 47] = [
    0x01, // version
    0x01, // kind: Admit
    // subject: 32 bytes, the base point
    0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x01, // via: Invite
    0x80, 0xb5, 0xd7, 0xd6, 0x06, // made_unix 1_792_400_000, LEB128
    0x06, 0x6c, 0x61, 0x70, 0x74, 0x6f, 0x70, // note: length 6, "laptop"
];

/// The made_unix of `Raw` values: the same 1_792_400_000 in LEB128.
const MADE_UNIX_LEB128: [u8; 5] = [0x80, 0xb5, 0xd7, 0xd6, 0x06];

// --- encode ---

#[test]
fn admit_value_starts_with_version_one() {
    // given: an admit value
    let value = admit_of(base());
    // when: encoded
    let bytes = encode(&value);
    // then: the first byte is the version, the second the kind
    assert_eq!(bytes[0], 1, "version byte");
    assert_eq!(bytes[1], 1, "kind byte: Admit = 1");
}

#[test]
fn golden_bytes_of_admit_v1() {
    // given: the fixed value of the golden constant
    let value = admit_of(base());
    // when: encoded
    let bytes = encode(&value);
    // then: the bytes equal the hand-laid constant, and the key is prefix plus lowercase hex
    assert_eq!(bytes, GOLDEN_ADMIT_V1, "any shift of a field, width or version shows here");
    let want_key = format!("acl/admit/58{}", "66".repeat(31));
    assert_eq!(admit_key(&base()), want_key.into_bytes());
}

#[test]
fn golden_bytes_decode_to_the_same_value() {
    // given: the golden bytes of version 1
    // when: decoded
    let decoded = decode(&GOLDEN_ADMIT_V1);
    // then: the value of the constant comes back
    assert_eq!(decoded, Ok(admit_of(base())));
}

// --- decode ---

#[test]
fn roundtrips_every_kind() {
    // given: every kind with every via, and an empty note and a note of 80 two-byte characters
    let notes = [String::new(), "é".repeat(80)];
    for (kind, via, note) in combinations(&notes) {
        let value = AclValue { kind, subject: base(), via, made_unix: 1_792_400_000, note };
        // when: encoded and decoded
        let back = decode(&encode(&value));
        // then: the same value (the note limit counts characters, not bytes)
        assert_eq!(back, Ok(value.clone()), "{value:?}");
    }
}

#[test]
fn roundtrips_extreme_timestamps() {
    for made_unix in [0, 127, 128, u64::MAX] {
        // given: a value with the timestamp at a varint width edge
        let value = AclValue { made_unix, ..admit_of(base()) };
        // when / then: it survives encoding
        assert_eq!(decode(&encode(&value)), Ok(value), "made_unix {made_unix}");
    }
}

#[test]
fn enums_from_u8_reject_unknown_bytes() {
    // given / when / then: known bytes map to their variants, the others to nothing
    assert_eq!(AclKind::from_u8(1), Some(AclKind::Admit));
    assert_eq!(AclKind::from_u8(4), Some(AclKind::Boot));
    assert_eq!((AclKind::from_u8(0), AclKind::from_u8(5)), (None, None));
    assert_eq!(Via::from_u8(0), Some(Via::None));
    assert_eq!(Via::from_u8(3), Some(Via::Witness));
    assert_eq!((Via::from_u8(4), Via::from_u8(255)), (None, None));
}

#[test]
fn ignores_unknown_version() {
    // given: empty bytes, and well-formed bodies under versions 0 and 2
    // when / then: empty is told apart from a wrong version
    assert_eq!(decode(&[]), Err(Ignored::Empty));
    for version in [0, 2, 255] {
        let bytes = raw(Raw { version, ..Raw::default() });
        assert_eq!(decode(&bytes), Err(Ignored::BadVersion), "version {version}");
    }
}

#[test]
fn ignores_unknown_kind() {
    // given: kinds outside 1..=4 and a via outside 0..=3 in otherwise good bodies
    // when / then: the kind and the via are told apart
    for kind in [0, 5, 255] {
        let bytes = raw(Raw { kind, ..Raw::default() });
        assert_eq!(decode(&bytes), Err(Ignored::UnknownKind), "kind {kind}");
    }
    let bytes = raw(Raw { via: 4, ..Raw::default() });
    assert_eq!(decode(&bytes), Err(Ignored::UnknownVia));
}

#[test]
fn ignores_trailing_bytes() {
    // given: a good value with one extra byte
    let mut bytes = encode(&admit_of(base()));
    bytes.push(0);
    // when / then: version 1 allows no tail
    assert_eq!(decode(&bytes), Err(Ignored::TrailingBytes));
}

#[test]
fn ignores_truncated_body() {
    // given: a good value without its last byte, and the version byte alone
    let mut bytes = encode(&admit_of(base()));
    bytes.pop();
    // when / then: the body is cut short
    assert_eq!(decode(&bytes), Err(Ignored::Malformed));
    assert_eq!(decode(&[1]), Err(Ignored::Malformed));
}

#[test]
fn ignores_non_utf8_note() {
    // given: a body whose note bytes are not UTF-8
    let bytes = raw(Raw { note: vec![0xFF, 0xFE], ..Raw::default() });
    // when / then: it is malformed
    assert_eq!(decode(&bytes), Err(Ignored::Malformed));
}

#[test]
fn ignores_note_over_80_chars() {
    // given: notes of 80 and 81 characters, and 81 two-byte characters
    let note = |s: String| raw(Raw { note: s.into_bytes(), ..Raw::default() });
    // when / then: 80 is fine, 81 is too long whatever the byte width
    assert!(decode(&note("a".repeat(80))).is_ok());
    assert!(decode(&note("é".repeat(80))).is_ok());
    assert!(decode(&note("a".repeat(64))).is_ok(), "an old note of 64 characters must still read");
    assert_eq!(decode(&note("a".repeat(81))), Err(Ignored::NoteTooLong));
    assert_eq!(decode(&note("é".repeat(81))), Err(Ignored::NoteTooLong));
}

#[test]
fn ignores_subject_off_curve() {
    // given: a body whose subject bytes are not a point of the curve
    let bytes = raw(Raw { subject: off_curve(), ..Raw::default() });
    // when / then: the subject is rejected as such, not as a generic postcard error
    assert_eq!(decode(&bytes), Err(Ignored::BadSubject));
}

// --- parse_entry ---

#[test]
fn record_carries_author_and_ts() {
    // given: an entry of A with a good admit value and a timestamp
    let (a, s) = (pid(2), pid(5));
    let e = entry(a, admit_key(&s), encode(&admit_of(s)), 123_456);
    // when: parsed
    let parsed = parse_entry(&e);
    // then: the record carries the author, the time and the value
    let want = Parsed::Record { author: a, ts_micros: 123_456, value: admit_of(s) };
    assert_eq!(parsed, want);
}

#[test]
fn tombstone_is_withdrawn() {
    // given: A's tombstone under the admit key of S
    let (a, s) = (pid(2), pid(5));
    let e = entry(a, admit_key(&s), vec![], 10);
    // when / then: the entry is a withdrawn record with the key
    assert_eq!(parse_entry(&e), Parsed::Withdrawn { author: a, key: admit_key(&s) });
}

#[test]
fn tombstone_under_foreign_key_is_foreign() {
    // given: a tombstone under a key outside the list
    let e = entry(pid(2), b"node/name/x".to_vec(), vec![], 10);
    // when / then: it is not a withdrawn record of the list
    assert_eq!(parse_entry(&e), Parsed::Ignored(Ignored::ForeignKey));
}

#[test]
fn subject_mismatch_with_key_is_ignored() {
    // given: A's values whose body disagrees with the key they lie under
    let (a, s, t) = (pid(2), pid(5), pid(6));
    let cases = [
        ("subject differs from key", admit_key(&t), admit_of(s)),
        ("kind differs from key", admit_key(&s), AclValue { kind: AclKind::Exclude, ..admit_of(s) }),
        ("boot marker for another subject", BOOT_KEY.to_vec(), AclValue { kind: AclKind::Boot, ..admit_of(s) }),
    ];
    for (name, key, value) in cases {
        // when / then: every one is a key mismatch
        let parsed = parse_entry(&entry(a, key, encode(&value), 10));
        assert_eq!(parsed, Parsed::Ignored(Ignored::KeyMismatch), "{name}");
    }
}

#[test]
fn self_exclusion_is_an_ordinary_record() {
    // given: A votes to exclude A
    let a = pid(2);
    let value = AclValue { kind: AclKind::Exclude, ..admit_of(a) };
    // when: parsed
    let parsed = parse_entry(&entry(a, exclude_key(&a), encode(&value), 10));
    // then: it is a record like any other vote
    assert_eq!(parsed, Parsed::Record { author: a, ts_micros: 10, value });
}

#[test]
fn boot_marker_of_its_author_is_a_record() {
    // given: A's boot marker naming A
    let a = pid(2);
    let value = AclValue { kind: AclKind::Boot, via: Via::Migration, ..admit_of(a) };
    // when: parsed
    let parsed = parse_entry(&entry(a, BOOT_KEY.to_vec(), encode(&value), 10));
    // then: it is accepted
    assert_eq!(parsed, Parsed::Record { author: a, ts_micros: 10, value });
}

#[test]
fn tagged_boot_marker_parses_as_a_boot_record() {
    // given: A's boot marker with the flag of the second admission stage
    let a = pid(2);
    let value = AclValue { kind: AclKind::Boot, via: Via::Migration, note: BOOT_NOTE_ADMISSION.to_string(), ..admit_of(a) };
    // when: parsed
    let parsed = parse_entry(&entry(a, BOOT_KEY.to_vec(), encode(&value), 10));
    // then: it is a boot record, which the tally does not count
    assert_eq!(parsed, Parsed::Record { author: a, ts_micros: 10, value });
    let (input, ignored) = tally_input(a, &[entry(a, BOOT_KEY.to_vec(), encode(&AclValue { kind: AclKind::Boot, ..admit_of(a) }), 10)]);
    assert!(input.admits.is_empty() && input.votes.is_empty() && ignored == 0);
}

#[test]
fn foreign_prefix_is_ignored() {
    // given: good values under keys that look like list keys but are not
    let s = pid(5);
    let hex = HEXLOWER.encode(s.as_bytes());
    let value = encode(&admit_of(s));
    let keys = [
        ("other namespace", b"node/name/x".to_vec()),
        ("upper-case hex", format!("acl/admit/{}", hex.to_uppercase()).into_bytes()),
        ("63 digits", format!("acl/admit/{}", &hex[..63]).into_bytes()),
        ("trailing slash", format!("acl/admit/{hex}/").into_bytes()),
        ("boot with a suffix", b"acl/bootx".to_vec()),
        ("admit prefix alone", b"acl/admit/".to_vec()),
        ("unknown kind in key", format!("acl/other/{hex}").into_bytes()),
    ];
    for (name, key) in keys {
        // when / then: the key alone makes the entry foreign
        let parsed = parse_entry(&entry(pid(2), key, value.clone(), 10));
        assert_eq!(parsed, Parsed::Ignored(Ignored::ForeignKey), "{name}");
    }
}

// --- keys ---

#[test]
fn keys_use_lowercase_hex() {
    // given: a node id and its hex written independently of `Display`
    let p = pid(5);
    let hex = HEXLOWER.encode(p.as_bytes());
    // when / then: keys are prefix plus 64 lowercase digits, without a trailing slash
    assert_eq!(admit_key(&p), format!("acl/admit/{hex}").into_bytes());
    assert_eq!(exclude_key(&p), format!("acl/exclude/{hex}").into_bytes());
    assert_eq!(block_key(&p), format!("acl-block/{hex}").into_bytes());
    assert_eq!(hex.len(), 64);
}

#[test]
fn key_for_picks_the_key_by_kind() {
    // given: a node id
    let p = pid(5);
    // when / then: every kind has its key; the boot marker ignores the subject
    assert_eq!(key_for(AclKind::Admit, &p), admit_key(&p));
    assert_eq!(key_for(AclKind::Exclude, &p), exclude_key(&p));
    assert_eq!(key_for(AclKind::Block, &p), block_key(&p));
    assert_eq!(key_for(AclKind::Boot, &p), BOOT_KEY);
    assert_eq!(BOOT_KEY, b"acl/boot");
}

#[test]
fn block_key_is_outside_the_shared_prefix() {
    // given: the prefixes and keys of a node
    let p = pid(5);
    // when / then: a scan of `acl/` sees admits, votes and the marker, never blocks
    assert!(admit_key(&p).starts_with(ACL_PREFIX) && exclude_key(&p).starts_with(ACL_PREFIX));
    assert!(BOOT_KEY.starts_with(ACL_PREFIX));
    assert!(!block_key(&p).starts_with(ACL_PREFIX));
    assert!(block_key(&p).starts_with(BLOCK_PREFIX));
}

// --- helpers ---

fn base_bytes() -> [u8; 32] {
    let mut bytes = [0x66; 32];
    bytes[0] = 0x58;
    bytes
}

/// The ed25519 base point: surely on the curve.
fn base() -> PeerId {
    PeerId::from_bytes(&base_bytes()).unwrap()
}

/// The first `[i; 32]` that is not a point of the curve.
fn off_curve() -> [u8; 32] {
    let bytes = (0..=255u8).map(|i| [i; 32]).find(|b| PeerId::from_bytes(b).is_err());
    bytes.expect("some [i; 32] must be off the curve")
}

/// The value of the golden test, for the given subject.
pub(super) fn admit_of(subject: PeerId) -> AclValue {
    AclValue {
        kind: AclKind::Admit,
        subject,
        via: Via::Invite,
        made_unix: 1_792_400_000,
        note: "laptop".to_string(),
    }
}

pub(super) fn entry(author: PeerId, key: Vec<u8>, value: Vec<u8>, ts_micros: u64) -> ReplicaEntry {
    ReplicaEntry { author, key, value: Bytes::from(value), ts_micros }
}

/// All kinds times all vias times the given notes.
fn combinations(notes: &[String]) -> Vec<(AclKind, Via, String)> {
    let kinds = [AclKind::Admit, AclKind::Exclude, AclKind::Block, AclKind::Boot];
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
pub(super) struct Raw {
    pub(super) version: u8,
    pub(super) kind: u8,
    pub(super) subject: [u8; 32],
    pub(super) via: u8,
    pub(super) note: Vec<u8>,
}

impl Default for Raw {
    fn default() -> Raw {
        Raw { version: 1, kind: 1, subject: base_bytes(), via: 1, note: b"laptop".to_vec() }
    }
}

pub(super) fn raw(r: Raw) -> Vec<u8> {
    let mut out = vec![r.version, r.kind];
    out.extend(r.subject);
    out.push(r.via);
    out.extend(MADE_UNIX_LEB128);
    out.extend(varint(r.note.len() as u64));
    out.extend(r.note);
    out
}

/// LEB128, as postcard writes lengths.
fn varint(mut n: u64) -> Vec<u8> {
    let mut out = vec![];
    while n >= 0x80 {
        out.push((n & 0x7F) as u8 | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
    out
}
