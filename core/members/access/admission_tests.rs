//! Tests of the admission count (`admission.rs`) through `tally`: one confirmation per newcomer,
//! the witness that is no vote at other nodes, requests that wait. Nodes: `m` is the own node.

use super::*;
use crate::members::access::tally::{tally, How, Ledger, Rules, TallyInput, VoteIn};
use crate::transport::net::contract::pid;

const N: Via = Via::None;
const I: Via = Via::Invite;
const W: Via = Via::Witness;

type Row = (PeerId, PeerId, Via);

fn input(me: PeerId, rows: &[Row]) -> TallyInput {
    let admits = rows.iter().map(|&(author, subject, via)| AdmitIn {
        author,
        subject,
        via,
        made_unix: 0,
    });
    TallyInput {
        me,
        admits: admits.collect(),
        votes: Vec::new(),
        forgets: Vec::new(),
        rules: Rules::Peer,
        powers: vec![],
    }
}

/// Every one of `ids` admits every other one.
fn full(ids: &[PeerId]) -> Vec<Row> {
    let pairs = ids.iter().flat_map(|a| ids.iter().map(move |b| (*a, *b)));
    pairs
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (a, b, N))
        .collect()
}

fn with(mut rows: Vec<Row>, more: &[Row]) -> Vec<Row> {
    rows.extend_from_slice(more);
    rows
}

fn ledger_of(me: PeerId, rows: &[Row]) -> Ledger {
    tally(&input(me, rows))
}

fn vote(voter: PeerId, subject: PeerId, ts_micros: u64) -> VoteIn {
    VoteIn {
        voter,
        subject,
        ts_micros,
        at_moment: false,
    }
}

fn short(voters: &[PeerId], need: u8) -> Admission {
    Admission {
        voters: voters.to_vec(),
        need,
        label: None,
    }
}

fn ids() -> (PeerId, PeerId, PeerId, PeerId, PeerId, PeerId) {
    (pid(1), pid(2), pid(3), pid(4), pid(9), pid(10))
}

fn set_of(ids: &[PeerId]) -> BTreeSet<PeerId> {
    ids.iter().copied().collect()
}

/// `full(m, a, b, c)`, a admits x, and `m` and `c` vote a out: a falls, the votes of a drop.
fn cascade_rows() -> (Vec<Row>, TallyInput) {
    let (m, a, b, c, x, _) = ids();
    let rows = with(full(&[m, a, b, c]), &[(a, x, I)]);
    let mut inp = input(m, &rows);
    inp.votes = vec![vote(m, a, 10), vote(c, a, 20)];
    (rows, inp)
}

#[test]
fn my_witness_admits_at_me() {
    // given: I write witnesses for a and c, a admits b
    let (m, a, b, c, ..) = ids();
    // when: tallied at m
    let ledger = ledger_of(m, &[(m, a, W), (m, c, W), (a, b, N)]);
    // then: a and c are in, and the single vote of a admits b
    assert_eq!(ledger.admitted, set_of(&[m, a, b, c]));
    assert!(ledger.admissions.is_empty());
}

#[test]
fn my_vote_counts_after_i_left() {
    // given: I admit a and leave
    let (m, a, ..) = ids();
    let mut inp = input(m, &[(m, a, N)]);
    inp.votes = vec![vote(m, m, 5)];
    // when: tallied
    let ledger = tally(&inp);
    // then: a is still a member and I am no request
    assert!(ledger.is_member(&a));
    assert!(!ledger.admissions.contains_key(&m));
}

#[test]
fn vote_of_one_member_admits_in_a_network_of_three() {
    // given: three members, only a invites x
    let (m, a, b, _, x, _) = ids();
    let rows = with(full(&[m, a, b]), &[(a, x, I)]);
    // when: tallied
    let ledger = ledger_of(m, &rows);
    // then: x is in and nothing waits
    assert!(ledger.admitted.contains(&x));
    assert!(ledger.admissions.is_empty());
}

#[test]
fn two_records_of_one_author_admit_once() {
    // given: a writes an invite and a confirmation for x
    let (m, a, b, _, x, _) = ids();
    let rows = with(full(&[m, a, b]), &[(a, x, I), (a, x, N)]);
    // when: tallied
    let ledger = ledger_of(m, &rows);
    // then: one vote is enough and nothing waits
    assert!(ledger.admitted.contains(&x));
    assert!(ledger.admissions.is_empty());
    let _ = a;
}

#[test]
fn witness_of_others_is_not_a_vote() {
    // given: a and b both write witnesses for x
    let (m, a, b, _, x, _) = ids();
    let rows = with(full(&[m, a, b]), &[(a, x, W), (b, x, W)]);
    // when: tallied
    let ledger = ledger_of(m, &rows);
    // then: x is not in, no vote counted
    assert!(!ledger.admitted.contains(&x));
    assert_eq!(ledger.admissions[&x], short(&[], 1));
    // and the exclusion threshold of the same network stays two
    assert_eq!(ledger.need, 2);
}

#[test]
fn newcomer_witness_cannot_launder() {
    // given: x is let in by a, then writes a witness for y
    let (m, a, b, _, x, y) = ids();
    let rows = with(full(&[m, a, b]), &[(a, x, I), (x, y, W)]);
    // when: tallied
    let ledger = ledger_of(m, &rows);
    // then: the witness is no vote, y waits
    assert!(!ledger.admitted.contains(&y));
    assert_eq!(ledger.admissions[&y], short(&[], 1));
}

#[test]
fn vote_of_a_pending_author_does_not_count() {
    // given: nobody admits x, x admits y
    let (m, a, b, _, x, y) = ids();
    let rows = with(full(&[m, a, b]), &[(x, y, N)]);
    // when: tallied
    let ledger = ledger_of(m, &rows);
    // then: neither is in and y is no request
    assert!(!ledger.admitted.contains(&x) && !ledger.admitted.contains(&y));
    assert!(!ledger.admissions.contains_key(&y));
}

#[test]
fn network_of_two_admits_on_one_vote() {
    // given: two members, a invites x
    let (m, a, _, _, x, _) = ids();
    // when: tallied
    let ledger = ledger_of(m, &with(full(&[m, a]), &[(a, x, I)]));
    // then: x is in
    assert!(ledger.admitted.contains(&x));
}

#[test]
fn two_simultaneous_requests_are_both_admitted() {
    // given: two members, a invites x and y at once
    let (m, a, _, _, x, y) = ids();
    // when: tallied
    let ledger = ledger_of(m, &with(full(&[m, a]), &[(a, x, I), (a, y, I)]));
    // then: both are in
    assert!(ledger.admitted.contains(&x) && ledger.admitted.contains(&y));
    assert!(ledger.admissions.is_empty());
}

#[test]
fn ledger_does_not_depend_on_record_order() {
    // given: two members, a invites x and y, x admits y
    let (m, a, _, _, x, y) = ids();
    let rows = with(full(&[m, a]), &[(a, x, I), (a, y, I), (x, y, N)]);
    let base = ledger_of(m, &rows);
    // when: the records come in 20 other orders
    for seed in 1..=20 {
        // then: the ledger is the same and both are in
        assert_eq!(ledger_of(m, &shuffled(&rows, seed)), base, "shuffle {seed}");
    }
    assert!(base.admitted.contains(&x) && base.admitted.contains(&y));
}

#[test]
fn excluded_author_vote_drops() {
    // given: x was let in by a alone, then a is excluded
    let (_, a, _, _, x, _) = ids();
    let (_, inp) = cascade_rows();
    // when: tallied
    let ledger = tally(&inp);
    // then: the vote of a is gone, x is out
    assert!(ledger.excluded.contains_key(&a) && !ledger.admitted.contains(&x));
}

#[test]
fn my_witness_keeps_member_after_cascade() {
    // given: the same cascade and my own witness for x
    let (m, _, _, _, x, _) = ids();
    let (_, mut inp) = cascade_rows();
    inp.admits.push(AdmitIn {
        author: m,
        subject: x,
        via: W,
        made_unix: 0,
    });
    // when: tallied
    let ledger = tally(&inp);
    // then: x stays a member and is no request
    assert!(ledger.is_member(&x));
    assert!(!ledger.admissions.contains_key(&x));
}

#[test]
fn excluded_subject_is_not_a_request() {
    // given: the same cascade, and x leaves before a falls
    let (_, _, _, _, x, _) = ids();
    let (_, mut inp) = cascade_rows();
    inp.votes.push(vote(x, x, 5));
    // when: tallied
    let ledger = tally(&inp);
    // then: x is excluded and shows up in no request
    assert_eq!(ledger.excluded[&x].how, How::Left);
    assert!(!ledger.admissions.contains_key(&x));
}

#[test]
fn newcomer_sees_its_own_request() {
    // given: I am x, I wrote witnesses for a and b, a admits me (before it writes nothing)
    let (_, a, b, c, x, _) = ids();
    let rows = with(full(&[a, b, c]), &[(x, a, W), (x, b, W), (a, x, I)]);
    // when: tallied at x
    let ledger = ledger_of(x, &rows);
    // then: the vote of a admits me and nothing waits
    assert!(ledger.is_member(&x));
    assert!(ledger.admissions.is_empty());
}

fn two_roots_rows(extra: &[Row]) -> Vec<Row> {
    let (_, a, b, c, d, _) = ids();
    with(
        vec![
            (a, b, N),
            (b, a, N),
            (a, c, N),
            (b, c, N),
            (a, d, N),
            (b, d, N),
        ],
        extra,
    )
}

fn two_roots_ledger(me: PeerId, rows: &[Row]) -> Ledger {
    let (_, a, b, _, d, _) = ids();
    let mut inp = input(me, rows);
    inp.votes = vec![vote(a, d, 10), vote(b, d, 20)];
    tally(&inp)
}

#[test]
fn two_roots_agree_when_every_member_has_two_votes() {
    // given: a and b see each other, c and d have two votes each; a and b vote d out
    let (_, a, b, _, d, _) = ids();
    let rows = two_roots_rows(&[]);
    // when: tallied from a and from b
    let (from_a, from_b) = (two_roots_ledger(a, &rows), two_roots_ledger(b, &rows));
    // then: the ledgers are equal and d is excluded
    assert_eq!(from_a, from_b);
    assert_eq!(from_a.excluded.keys().copied().collect::<Vec<_>>(), vec![d]);
}

#[test]
fn two_roots_agree_on_a_single_vote() {
    // given: as above, but c is vouched for by a alone and e by both
    let (_, a, b, c, d, e) = (pid(1), pid(2), pid(3), pid(4), pid(5), pid(6));
    let rows = with(
        vec![(a, b, N), (b, a, N), (a, c, N), (a, d, N), (b, d, N)],
        &[(a, e, N), (b, e, N)],
    );
    let vote_out = |me| {
        let mut inp = input(me, &rows);
        inp.votes = vec![vote(a, d, 10), vote(b, d, 20)];
        tally(&inp)
    };
    // when: tallied from a and from b
    let (from_a, from_b) = (vote_out(a), vote_out(b));
    // then: the exclusions agree; c is in at both
    assert_eq!(from_a.excluded, from_b.excluded);
    assert!(from_a.admitted.contains(&c) && from_b.admitted.contains(&c));
}

#[test]
fn admit_chain_of_single_votes() {
    // given: m admits a and d, a admits b, b admits c
    let (m, a, b, c, d, _) = (pid(1), pid(2), pid(3), pid(4), pid(5), pid(6));
    // when: tallied
    let ledger = ledger_of(m, &[(m, a, N), (a, b, N), (b, c, N), (m, d, N)]);
    // then: each vote admits the next, and the exclusion need stays two
    assert_eq!(ledger.admitted, set_of(&[m, a, b, c, d]));
    assert!(ledger.admissions.is_empty());
    assert_eq!(ledger.need, 2);
}

/// Deterministic Fisher-Yates shuffle (xorshift), so a failure repeats.
fn shuffled<T: Clone>(items: &[T], seed: u64) -> Vec<T> {
    let mut out = items.to_vec();
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    for i in (1..out.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.swap(i, (state % (i as u64 + 1)) as usize);
    }
    out
}
