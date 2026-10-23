//! Tests of building the tally input from replica entries (`tally_input` in `record_input.rs`).
//! Helpers for entries and raw values come from `record_tests.rs`.

use super::super::record::tests::{admit_of, entry, raw, Raw};
use super::super::record::{admit_key, encode, exclude_key, AclKind, AclValue};
use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

#[test]
fn live_records_become_admits_and_votes() {
    // given: A admits B (live) and A votes against B at 200
    let (me, a, b) = (pid(1), pid(2), pid(3));
    let vote = AclValue {
        kind: AclKind::Exclude,
        ..admit_of(b)
    };
    let entries = [
        entry(a, admit_key(&b), encode(&admit_of(b)), 100),
        entry(a, exclude_key(&b), encode(&vote), 200),
    ];
    // when: the input is built
    let (input, ignored) = tally_input(me, &entries);
    // then: one admit row, one vote row with the entry's time, nothing ignored
    assert_eq!(input.me, me);
    assert_eq!(admit_rows(&input), [(a, b)]);
    assert_eq!(vote_rows(&input), [(a, b, 200)]);
    assert_eq!(ignored, 0);
}

#[test]
fn withdrawn_self_exclusion_stays_left() {
    // given: A's tombstone under A's own self-exclusion key (A left and took the vote back)
    let (me, a) = (pid(1), pid(2));
    let entries = [entry(a, exclude_key(&a), vec![], 300)];
    // when: the input is built
    let (input, ignored) = tally_input(me, &entries);
    // then: it still counts as a live vote of A against A (leaving is irreversible)
    assert_eq!(vote_rows(&input), [(a, a, 300)]);
    assert_eq!(ignored, 0);
}

#[test]
fn withdrawn_vote_on_other_is_dropped() {
    // given: A's tombstones under the vote key and the admit key of B
    let (me, a, b) = (pid(1), pid(2), pid(3));
    let entries = [
        entry(a, exclude_key(&b), vec![], 300),
        entry(a, admit_key(&b), vec![], 310),
    ];
    // when: the input is built
    let (input, ignored) = tally_input(me, &entries);
    // then: a withdrawn vote on someone else is gone, and so is a withdrawn admit; neither is garbage
    assert!(vote_rows(&input).is_empty() && admit_rows(&input).is_empty());
    assert_eq!(ignored, 0);
}

#[test]
fn garbage_counts_as_ignored() {
    // given: one good admit and three bad entries: wrong version, truncated body, key mismatch
    let (me, a, b, c, d, e) = (pid(1), pid(2), pid(3), pid(4), pid(5), pid(6));
    let mut cut = encode(&admit_of(d));
    cut.pop();
    let entries = [
        entry(a, admit_key(&b), encode(&admit_of(b)), 100),
        entry(
            a,
            admit_key(&c),
            raw(Raw {
                version: 9,
                ..Raw::default()
            }),
            110,
        ),
        entry(a, admit_key(&d), cut, 120),
        entry(a, admit_key(&e), encode(&admit_of(b)), 130),
    ];
    // when: the input is built
    let (input, ignored) = tally_input(me, &entries);
    // then: only the good admit remains, and the three bad ones are counted
    assert_eq!(ignored, 3, "three entries are garbage");
    assert_eq!(admit_rows(&input), [(a, b)]);
}

// --- helpers ---

fn admit_rows(input: &TallyInput) -> Vec<(PeerId, PeerId)> {
    input.admits.iter().map(|a| (a.author, a.subject)).collect()
}

fn vote_rows(input: &TallyInput) -> Vec<(PeerId, PeerId, u64)> {
    input
        .votes
        .iter()
        .map(|v| (v.voter, v.subject, v.ts_micros))
        .collect()
}
