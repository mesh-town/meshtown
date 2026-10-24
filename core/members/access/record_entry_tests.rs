//! Tests of parsing replica entries into records, tombstones and boot markers, and of the keys
//! (`record.rs`).

use super::support_tests::*;
use super::*;
use crate::transport::net::contract::pid;
use data_encoding::HEXLOWER;

// --- parse_entry ---

#[test]
fn record_carries_author_and_ts() {
    // given: an entry of A with a good admit value and a timestamp
    let (a, s) = (pid(2), pid(5));
    let e = entry(a, admit_key(&s), encode(&admit_of(s)), 123_456);
    // when: parsed
    let parsed = parse_entry(&e);
    // then: the record carries the author, the time and the value
    let want = Parsed::Record {
        author: a,
        ts_micros: 123_456,
        value: admit_of(s),
    };
    assert_eq!(parsed, want);
}

#[test]
fn tombstone_is_withdrawn() {
    // given: A's tombstone under the admit key of S
    let (a, s) = (pid(2), pid(5));
    let e = entry(a, admit_key(&s), vec![], 10);
    // when / then: the entry is a withdrawn record with the key
    assert_eq!(
        parse_entry(&e),
        Parsed::Withdrawn {
            author: a,
            key: admit_key(&s)
        }
    );
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
        (
            "kind differs from key",
            admit_key(&s),
            AclValue {
                kind: AclKind::Exclude,
                ..admit_of(s)
            },
        ),
        (
            "boot marker for another subject",
            BOOT_KEY.to_vec(),
            AclValue {
                kind: AclKind::Boot,
                ..admit_of(s)
            },
        ),
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
    let value = AclValue {
        kind: AclKind::Exclude,
        ..admit_of(a)
    };
    // when: parsed
    let parsed = parse_entry(&entry(a, exclude_key(&a), encode(&value), 10));
    // then: it is a record like any other vote
    assert_eq!(
        parsed,
        Parsed::Record {
            author: a,
            ts_micros: 10,
            value
        }
    );
}

#[test]
fn boot_marker_of_its_author_is_a_record() {
    // given: A's boot marker naming A
    let a = pid(2);
    let value = AclValue {
        kind: AclKind::Boot,
        via: Via::Migration,
        ..admit_of(a)
    };
    // when: parsed
    let parsed = parse_entry(&entry(a, BOOT_KEY.to_vec(), encode(&value), 10));
    // then: it is accepted
    assert_eq!(
        parsed,
        Parsed::Record {
            author: a,
            ts_micros: 10,
            value
        }
    );
}

#[test]
fn tagged_boot_marker_parses_as_a_boot_record() {
    // given: A's boot marker with the flag of the second admission stage
    let a = pid(2);
    let value = AclValue {
        kind: AclKind::Boot,
        via: Via::Migration,
        note: BOOT_NOTE_ADMISSION.to_string(),
        ..admit_of(a)
    };
    // when: parsed
    let parsed = parse_entry(&entry(a, BOOT_KEY.to_vec(), encode(&value), 10));
    // then: it is a boot record, which the tally does not count
    assert_eq!(
        parsed,
        Parsed::Record {
            author: a,
            ts_micros: 10,
            value
        }
    );
    let (input, ignored) = tally_input(
        a,
        &[entry(
            a,
            BOOT_KEY.to_vec(),
            encode(&AclValue {
                kind: AclKind::Boot,
                ..admit_of(a)
            }),
            10,
        )],
    );
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
        (
            "upper-case hex",
            format!("acl/admit/{}", hex.to_uppercase()).into_bytes(),
        ),
        (
            "63 digits",
            format!("acl/admit/{}", &hex[..63]).into_bytes(),
        ),
        ("trailing slash", format!("acl/admit/{hex}/").into_bytes()),
        ("boot with a suffix", b"acl/bootx".to_vec()),
        ("admit prefix alone", b"acl/admit/".to_vec()),
        (
            "unknown kind in key",
            format!("acl/other/{hex}").into_bytes(),
        ),
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

// --- grant, revoke, close ---

#[test]
fn grant_revoke_close_round_trip() {
    // given: A's records of the three kinds of administration; a close names its author
    let (a, s) = (pid(2), pid(5));
    let hex = HEXLOWER.encode(s.as_bytes());
    let cases = [
        (
            AclKind::Grant,
            6,
            s,
            format!("acl/admin/{hex}").into_bytes(),
        ),
        (
            AclKind::Revoke,
            7,
            s,
            format!("acl/unadmin/{hex}").into_bytes(),
        ),
        (AclKind::Close, 8, a, b"acl/close".to_vec()),
    ];
    for (kind, byte, subject, want_key) in cases {
        let value = AclValue {
            kind,
            subject,
            ..admit_of(subject)
        };
        // when: the value is encoded and the entry under the key of its kind is parsed
        let key = key_for(kind, &subject);
        let bytes = encode(&value);
        let parsed = parse_entry(&entry(a, key.clone(), bytes.clone(), 10));
        // then: the kind byte and the key are fixed, and the record comes back whole
        assert_eq!(bytes[1], byte, "{kind:?}: kind byte");
        assert_eq!(AclKind::from_u8(byte), Some(kind));
        assert_eq!(key, want_key, "{kind:?}: key");
        let want = Parsed::Record {
            author: a,
            ts_micros: 10,
            value,
        };
        assert_eq!(parsed, want, "{kind:?}");
    }
}

#[test]
fn close_with_foreign_subject_is_key_mismatch() {
    // given: A's close record that names S, not A
    let (a, s) = (pid(2), pid(5));
    let value = AclValue {
        kind: AclKind::Close,
        ..admit_of(s)
    };
    // when / then: a close is the author's own record, so it does not match its key
    let parsed = parse_entry(&entry(a, key_for(AclKind::Close, &a), encode(&value), 10));
    assert_eq!(parsed, Parsed::Ignored(Ignored::KeyMismatch));
}

#[test]
fn grant_body_under_admit_key_is_key_mismatch() {
    // given: A's values whose kind disagrees with the key of another kind of administration
    let (a, s) = (pid(2), pid(5));
    let of = |kind| AclValue {
        kind,
        ..admit_of(s)
    };
    let cases = [
        (
            "grant under the admit key",
            admit_key(&s),
            of(AclKind::Grant),
        ),
        (
            "revoke under the grant key",
            grant_key(&s),
            of(AclKind::Revoke),
        ),
        (
            "admit under the revoke key",
            revoke_key(&s),
            of(AclKind::Admit),
        ),
    ];
    for (name, key, value) in cases {
        // when / then: every one is a key mismatch
        let parsed = parse_entry(&entry(a, key, encode(&value), 10));
        assert_eq!(parsed, Parsed::Ignored(Ignored::KeyMismatch), "{name}");
    }
}
