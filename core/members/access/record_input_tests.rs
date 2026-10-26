//! Tests of building the tally input from replica entries (`tally_input` in `record_input.rs`).
//! Helpers for entries and raw values come from `record_tests.rs`.

use super::super::record::support_tests::{admit_of, entry, raw, Raw};
use super::super::record::{admit_key, encode, exclude_key, AclKind, AclValue};
use super::*;
use crate::members::access::authority::{Power, PowerIn};
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

#[test]
fn live_records_become_powers_and_votes() {
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
    let (input, ignored) = tally_input(root(), me, &entries);
    // then: one admission, one vote row with the entry's time, nothing ignored
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
    let (input, ignored) = tally_input(root(), me, &entries);
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
    let (input, ignored) = tally_input(root(), me, &entries);
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
    let (input, ignored) = tally_input(root(), me, &entries);
    // then: only the good admit remains, and the three bad ones are counted
    assert_eq!(ignored, 3, "three entries are garbage");
    assert_eq!(admit_rows(&input), [(a, b)]);
}

mod powers {
    use super::*;
    use crate::members::access::record::{grant_key, revoke_key, Via, CLOSE_KEY};

    #[test]
    fn powers_from_records() {
        // given: A admits B by an invitation and by a witness record, makes B an admin, takes the role
        // back, and closes the network; the records are written at 100..=500 us
        let (me, a, b) = (pid(1), pid(2), pid(3));
        let value = |kind, subject| AclValue {
            kind,
            subject,
            ..admit_of(subject)
        };
        let witness = AclValue {
            via: Via::Witness,
            ..admit_of(pid(4))
        };
        let entries = [
            entry(a, admit_key(&b), encode(&admit_of(b)), 100),
            entry(a, admit_key(&pid(4)), encode(&witness), 150),
            entry(a, grant_key(&b), encode(&value(AclKind::Grant, b)), 200),
            entry(a, revoke_key(&b), encode(&value(AclKind::Revoke, b)), 300),
            entry(
                a,
                CLOSE_KEY.to_vec(),
                encode(&value(AclKind::Close, a)),
                500,
            ),
        ];
        // when: the input is built
        let (input, ignored) = tally_input(root(), me, &entries);
        // then: every record is a record of power with the time of the entry (not the time inside
        // the value); an admission is one of them
        let power = |power, subject, ts_micros| PowerIn {
            author: a,
            power,
            subject,
            ts_micros,
        };
        let want = vec![
            power(Power::Admit(Via::Invite), b, 100),
            power(Power::Admit(Via::Witness), pid(4), 150),
            power(Power::Grant, b, 200),
            power(Power::Revoke, b, 300),
            power(Power::Close, a, 500),
        ];
        assert_eq!(input.powers, want);
        assert_eq!(admit_rows(&input), [(a, b), (a, pid(4))]);
        assert_eq!(ignored, 0);
    }

    #[test]
    fn power_tombstones_are_absent() {
        // given: A's tombstones under the grant, revoke, close and admit keys
        let (me, a, b) = (pid(1), pid(2), pid(3));
        let entries = [
            entry(a, grant_key(&b), vec![], 200),
            entry(a, revoke_key(&b), vec![], 210),
            entry(a, CLOSE_KEY.to_vec(), vec![], 220),
            entry(a, admit_key(&b), vec![], 230),
        ];
        // when: the input is built
        let (input, ignored) = tally_input(root(), me, &entries);
        // then: a taken-back record is not a record of power; nobody counts it as garbage
        assert!(input.powers.is_empty() && admit_rows(&input).is_empty());
        assert_eq!(ignored, 0);
    }
}

// --- helpers ---

fn root() -> PeerId {
    pid(9)
}

fn admit_rows(input: &TallyInput) -> Vec<(PeerId, PeerId)> {
    let admit = |p: &PowerIn| matches!(p.power, Power::Admit(_)).then_some((p.author, p.subject));
    input.powers.iter().filter_map(admit).collect()
}

fn vote_rows(input: &TallyInput) -> Vec<(PeerId, PeerId, u64)> {
    input
        .votes
        .iter()
        .map(|v| (v.voter, v.subject, v.ts_micros))
        .collect()
}
