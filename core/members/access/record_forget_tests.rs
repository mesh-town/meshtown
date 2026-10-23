//! Tests of the forget vote record (`acl/forget/<hex>`): kind, key, value bytes, parsing of
//! replica entries and the forget rows of the tally input. Helpers for entries and values come
//! from `record_tests.rs`.

use super::super::tally::{ForgetIn, TallyInput};
use super::tests::{admit_of, entry};
use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

mod kind_and_key {
    use super::*;

    #[test]
    fn forget_kind_is_five_and_round_trips() {
        assert_eq!(AclKind::Forget as u8, 5, "wire number of the forget kind");
        assert_eq!(AclKind::from_u8(5), Some(AclKind::Forget));
        assert_eq!(AclKind::from_u8(6), None, "no kind beyond forget");
    }

    #[test]
    fn forget_key_has_forget_prefix() {
        // given: a subject
        let p = pid(2);
        // when: the forget key is built
        let key = forget_key(&p);
        // then: it is the prefix plus the lowercase hex of the subject, and differs from the other keys
        assert_eq!(key, format!("acl/forget/{p}").into_bytes());
        assert_eq!(key_for(AclKind::Forget, &p), key, "key_for agrees");
        assert_ne!(key, exclude_key(&p));
        assert_ne!(key, admit_key(&p));
    }

    #[test]
    fn forget_key_parses_back_to_kind_and_subject() {
        let p = pid(2);
        assert_eq!(parse_key(&forget_key(&p)), Some((AclKind::Forget, Some(p))));
    }

    #[test]
    fn forget_prefix_with_junk_is_not_a_key() {
        // given: the forget key with a stray byte, and the bare prefix
        let mut junk = forget_key(&pid(2));
        junk.push(b'x');
        // then: neither is a record key
        assert_eq!(parse_key(&junk), None, "trailing byte");
        assert_eq!(parse_key(b"acl/forget/"), None, "no subject");
    }

    #[test]
    fn forget_value_encodes_kind_byte_five() {
        // given: a forget value
        let v = forget_value(pid(2), 1_000);
        // when: encoded and decoded
        let bytes = encode(&v);
        // then: the kind byte is 5 and the value comes back unchanged
        assert_eq!(bytes[1], 5, "kind byte: Forget = 5");
        assert_eq!(decode(&bytes), Ok(v));
    }
}

mod parse {
    use super::*;

    #[test]
    fn forget_record_parses_into_forget_in() {
        // given: A's live forget record about B, made at 1000 and stored at replica time 5_000_000
        let (me, a, b) = (pid(1), pid(2), pid(3));
        let entries = [forget_entry(a, b, 1_000, 5_000_000)];
        // when: the input is built
        let (input, ignored) = tally_input(me, &entries);
        // then: one forget row with the time of the body (not of the replica), nothing else, nothing ignored
        assert_eq!(
            input.forgets,
            [ForgetIn {
                author: a,
                subject: b,
                made_unix: 1_000
            }]
        );
        assert!(input.admits.is_empty() && input.votes.is_empty());
        assert_eq!(ignored, 0);
    }

    #[test]
    fn forget_entry_is_a_record_of_kind_forget() {
        let (a, b) = (pid(2), pid(3));
        let parsed = parse_entry(&forget_entry(a, b, 1_000, 10));
        assert_eq!(
            parsed,
            Parsed::Record {
                author: a,
                ts_micros: 10,
                value: forget_value(b, 1_000)
            }
        );
    }

    #[test]
    fn forget_tombstone_is_withdrawn_not_counted() {
        // given: A's tombstone under the forget key of B, and under the forget key of A itself
        let (me, a, b) = (pid(1), pid(2), pid(3));
        let entries = [
            entry(a, forget_key(&b), vec![], 300),
            entry(a, forget_key(&a), vec![], 310),
        ];
        // when: the entry is classified and the input is built
        let parsed = parse_entry(&entries[0]);
        let (input, ignored) = tally_input(me, &entries);
        // then: it is a withdrawal; no forget row, no exclusion row (unlike a self-exclusion), no garbage
        assert_eq!(
            parsed,
            Parsed::Withdrawn {
                author: a,
                key: forget_key(&b)
            }
        );
        assert!(input.forgets.is_empty(), "withdrawn votes do not count");
        assert!(
            input.votes.is_empty(),
            "a forget tombstone is no exclusion vote"
        );
        assert_eq!(ignored, 0);
    }

    #[test]
    fn forget_key_with_other_subject_is_key_mismatch() {
        // given: a forget value about C under the forget key of B
        let (me, a, b, c) = (pid(1), pid(2), pid(3), pid(4));
        let entries = [entry(
            a,
            forget_key(&b),
            encode(&forget_value(c, 1_000)),
            10,
        )];
        // when: classified and counted
        let parsed = parse_entry(&entries[0]);
        let (input, ignored) = tally_input(me, &entries);
        // then: the entry is garbage and gives no row
        assert_eq!(parsed, Parsed::Ignored(Ignored::KeyMismatch));
        assert!(input.forgets.is_empty());
        assert_eq!(ignored, 1);
    }

    #[test]
    fn forget_value_under_exclude_key_is_key_mismatch() {
        let (a, b) = (pid(2), pid(3));
        let e = entry(a, exclude_key(&b), encode(&forget_value(b, 1_000)), 10);
        assert_eq!(parse_entry(&e), Parsed::Ignored(Ignored::KeyMismatch));
    }

    #[test]
    fn exclude_value_under_forget_key_is_key_mismatch() {
        let (a, b) = (pid(2), pid(3));
        let vote = AclValue {
            kind: AclKind::Exclude,
            ..forget_value(b, 1_000)
        };
        let e = entry(a, forget_key(&b), encode(&vote), 10);
        assert_eq!(parse_entry(&e), Parsed::Ignored(Ignored::KeyMismatch));
    }

    #[test]
    fn truncated_forget_body_counts_as_ignored() {
        // given: a forget record with the last byte cut off
        let (me, a, b) = (pid(1), pid(2), pid(3));
        let mut cut = encode(&forget_value(b, 1_000));
        cut.pop();
        // when: the input is built
        let (input, ignored) = tally_input(me, &[entry(a, forget_key(&b), cut, 10)]);
        // then: garbage, no row
        assert!(input.forgets.is_empty());
        assert_eq!(ignored, 1);
    }

    #[test]
    fn votes_of_several_authors_all_become_rows() {
        // given: A and B both vote to forget C, and A also votes about D
        let (me, a, b, c, d) = (pid(1), pid(2), pid(3), pid(4), pid(5));
        let entries = [
            forget_entry(a, c, 100, 1),
            forget_entry(b, c, 200, 2),
            forget_entry(a, d, 300, 3),
        ];
        // when: the input is built
        let (input, ignored) = tally_input(me, &entries);
        // then: one row per record (pair of author and subject), in entry order
        assert_eq!(forget_rows(&input), [(a, c, 100), (b, c, 200), (a, d, 300)]);
        assert_eq!(ignored, 0);
    }

    #[test]
    fn forget_record_is_not_an_admit_or_an_exclusion() {
        // given: A's forget record about A itself (a self vote)
        let (me, a) = (pid(1), pid(2));
        let (input, _) = tally_input(me, &[forget_entry(a, a, 100, 1)]);
        // then: it stays a forget row only; leaving (self-exclusion) is a different record
        assert!(input.admits.is_empty() && input.votes.is_empty());
    }
}

mod admit_rows {
    use super::*;

    #[test]
    fn admit_in_carries_made_unix() {
        // given: A's live admit of B made at 1_792_400_000 and of C made at 42 (replica times differ)
        let (me, a, b, c) = (pid(1), pid(2), pid(3), pid(4));
        let early = AclValue {
            made_unix: 42,
            ..admit_of(c)
        };
        let entries = [
            entry(a, admit_key(&b), encode(&admit_of(b)), 900),
            entry(a, admit_key(&c), encode(&early), 100),
        ];
        // when: the input is built
        let (input, ignored) = tally_input(me, &entries);
        // then: each admit row has the time of its body
        let rows: Vec<_> = input
            .admits
            .iter()
            .map(|r| (r.author, r.subject, r.made_unix))
            .collect();
        assert_eq!(rows, [(a, b, 1_792_400_000), (a, c, 42)]);
        assert_eq!(ignored, 0);
    }

    #[test]
    fn admit_keeps_its_via_next_to_made_unix() {
        // given: a witness admit made at 77
        let (me, a, b) = (pid(1), pid(2), pid(3));
        let witness = AclValue {
            via: Via::Witness,
            made_unix: 77,
            ..admit_of(b)
        };
        // when: the input is built
        let (input, _) = tally_input(me, &[entry(a, admit_key(&b), encode(&witness), 1)]);
        // then: via and time are both on the row
        assert_eq!(input.admits.len(), 1);
        assert_eq!(
            (input.admits[0].via, input.admits[0].made_unix),
            (Via::Witness, 77)
        );
    }
}

// --- helpers ---

fn forget_value(subject: PeerId, made_unix: u64) -> AclValue {
    AclValue {
        kind: AclKind::Forget,
        subject,
        via: Via::None,
        made_unix,
        note: String::new(),
    }
}

/// A live forget record of `author` about `subject`, stored at replica time `ts_micros`.
fn forget_entry(author: PeerId, subject: PeerId, made_unix: u64, ts_micros: u64) -> ReplicaEntry {
    let value = encode(&forget_value(subject, made_unix));
    entry(author, forget_key(&subject), value, ts_micros)
}

fn forget_rows(input: &TallyInput) -> Vec<(PeerId, PeerId, u64)> {
    input
        .forgets
        .iter()
        .map(|f| (f.author, f.subject, f.made_unix))
        .collect()
}
