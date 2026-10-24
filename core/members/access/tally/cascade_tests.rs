//! Tests of the membership list tally (`pass.rs`, `votes.rs`), part two: cascade after an exclusion,
//! determinism (order of input, ties, the round cap) and attack scenarios on 2, 3 and 5 nodes.
//! Helpers (`cast`, `scene`, `members`, ...) come from `tests.rs`.

use super::tests::{by_votes, cast, excluded, left, members, scene, set, waiting, Cast, Scene};
use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;
use std::collections::BTreeMap;

// --- cascade ---

#[test]
fn admits_by_excluded_author_stop_counting() {
    // given: M admits A and B, A admits D; M and B vote against A
    let Cast { m, a, b, d, .. } = cast();
    let s = members(m, &[a, b])
        .admit(a, d)
        .vote(m, a, 10)
        .vote(b, a, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: A is out, and D, admitted by A alone, is neither listed nor excluded
    assert_eq!(ledger.excluded, excluded(&[(a, by_votes(20, &[m, b]))]));
    assert!(!ledger.admitted.contains(&d) && !ledger.excluded.contains_key(&d));
}

#[test]
fn second_admitter_keeps_subject() {
    // given: the same, but B admitted D as well
    let Cast { m, a, b, d, .. } = cast();
    let s = members(m, &[a, b]).admit(a, d).admit(b, d);
    let s = s.vote(m, a, 10).vote(b, a, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: D stays a member
    assert!(!ledger.is_member(&a));
    assert!(ledger.is_member(&d));
}

#[test]
fn exclusion_survives_admitter_exclusion() {
    // given: A admits D; D leaves at 10, then B and C exclude A at 20 and 30
    let Cast { m, a, b, c, d, .. } = cast();
    let s = members(m, &[a, b, c]).admit(a, d);
    let s = s.vote(d, d, 10).vote(b, a, 20).vote(c, a, 30);
    // when: tallied
    let ledger = s.ledger();
    // then: D is no longer reachable from M, yet stays recorded as excluded, not "unlisted"
    assert!(!ledger.admitted.contains(&d));
    assert_eq!(ledger.excluded.get(&d), Some(&left(10, d)));
    assert!(!ledger.is_member(&d));
}

#[test]
fn voter_dropped_by_cascade_keeps_past_exclusion() {
    // given: Y admitted X, M admitted W; X and Z vote against Y (an input that makes a naive fixed point flip)
    let Cast {
        m,
        a: y,
        b: x,
        c: z,
        d: w,
        ..
    } = cast();
    let s = members(m, &[y, z])
        .admit(y, x)
        .admit(m, w)
        .vote(x, y, 10)
        .vote(z, y, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: the vote of X was lawful when cast, so Y is excluded though X dropped out afterwards
    assert_eq!(ledger.excluded, excluded(&[(y, by_votes(20, &[x, z]))]));
    assert!(!ledger.admitted.contains(&x) && !ledger.is_member(&x));
    assert_eq!(s.ledger(), ledger, "a second run gives the same ledger");
}

#[test]
fn my_admits_count_after_i_left() {
    // given: M admits A and B, then M leaves
    let Cast { m, a, b, .. } = cast();
    let s = members(m, &[a, b]).vote(m, m, 10);
    // when: tallied
    let ledger = s.ledger();
    // then: M is excluded as having left, but A and B, admitted by M, remain members
    assert_eq!(ledger.excluded, excluded(&[(m, left(10, m))]));
    assert!(ledger.is_member(&a) && ledger.is_member(&b) && !ledger.is_member(&m));
}

// --- determinism ---

#[test]
fn shuffled_input_gives_same_ledger() {
    // given: ten nodes, eleven admits, thirty votes with ties in time and repeated pairs
    let (admits, votes) = crowd();
    let base = crowd_scene(&admits, &votes).ledger();
    assert!(
        !base.excluded.is_empty() && !base.pending.is_empty(),
        "scenario must exercise both"
    );
    for seed in 1..=20 {
        // when: both record lists are shuffled
        let (admits, votes) = (shuffled(&admits, seed), shuffled(&votes, seed + 100));
        // then: the ledger is the same
        assert_eq!(
            crowd_scene(&admits, &votes).ledger(),
            base,
            "shuffle {seed}"
        );
    }
}

#[test]
fn equal_timestamps_ordered_by_author_bytes() {
    // given: two members vote against each other with the same timestamp
    let (p, q) = (pid(2), pid(3));
    let (lo, hi) = (p.min(q), p.max(q));
    for (me, other) in [(p, q), (q, p)] {
        // when: tallied from either side
        let ledger = members(me, &[other])
            .vote(lo, hi, 5)
            .vote(hi, lo, 5)
            .ledger();
        // then: the voter with smaller bytes went first, so the one it targeted is excluded
        assert_eq!(
            ledger.excluded,
            excluded(&[(hi, by_votes(5, &[lo]))]),
            "me = {me:?}"
        );
        assert!(ledger.is_member(&lo));
    }
}

#[test]
fn duplicate_vote_counts_once() {
    // given: three members; A votes against B twice, at 10 and at 20
    let Cast { m, a, b, .. } = cast();
    let s = members(m, &[a, b]).vote(a, b, 10).vote(a, b, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: B waits with a single vote
    assert_eq!(ledger.pending, BTreeMap::from([(b, waiting(&[a], 2))]));
    assert!(ledger.is_member(&b));
}

#[test]
fn stops_at_round_cap() {
    // given: five members; B and D each have two valid waiting votes (as if the threshold had just dropped)
    let mut pass = pass_with_two_candidates();
    let Cast { a, b, c, d, .. } = cast();
    // when: the settling gets room for one round only
    let finished = pass.settle(30, 1);
    // then: the cap is reported, the earlier candidate B is excluded, D still waits
    assert!(!finished, "settling must report that it ran into the cap");
    assert_eq!(pass.excluded, excluded(&[(b, by_votes(30, &[a, c]))]));
    assert!(
        pass.pending.contains_key(&d),
        "the second candidate stays in pending"
    );
}

#[test]
fn settle_with_enough_rounds_excludes_both_candidates() {
    // given: the same two candidates
    let mut pass = pass_with_two_candidates();
    let Cast { a, b, c, d, .. } = cast();
    // when: the settling gets room for all rounds
    let finished = pass.settle(30, 10);
    // then: it finishes, and both are excluded, the earlier first-voter's candidate first
    assert!(finished);
    let want = excluded(&[(b, by_votes(30, &[a, c])), (d, by_votes(30, &[a, c]))]);
    assert_eq!(pass.finish().excluded, want);
}

// --- attacks ---

#[test]
fn witness_keeps_invitee_of_excluded_admitter() {
    // given: me = A; A admits M, B and X (a witness record), M admits X; A and B vote against M
    let Cast { m, a, b, x, .. } = cast();
    let s = members(a, &[m, b, x])
        .admit(m, x)
        .vote(a, m, 10)
        .vote(b, m, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: M is out, X stays a member through A's own record
    assert_eq!(ledger.excluded, excluded(&[(m, by_votes(20, &[a, b]))]));
    assert!(ledger.is_member(&x));
}

#[test]
fn backdated_vote_in_pair_beats_later_vote() {
    // given: me = M admits L; M votes against L at 100, L votes against M at 50 (written later, dated earlier)
    let Cast { m, a: l, .. } = cast();
    let s = members(m, &[l]).vote(m, l, 100).vote(l, m, 50);
    // when: tallied
    let ledger = s.ledger();
    // then: the backdated vote wins: M is excluded and its own vote does not count (known risk)
    assert_eq!(ledger.excluded, excluded(&[(m, by_votes(50, &[l]))]));
    assert!(ledger.is_member(&l));
}

#[test]
fn colluding_pair_backdates_against_proposer_in_five() {
    // given: me = B admits A, C, M, X; M and X exclude A at 20; A votes against M at 100 (backdated); B at 200
    let Cast { m, a, b, c, x, .. } = cast();
    let s = members(b, &[a, c, m, x]);
    let s = s
        .vote(m, a, 10)
        .vote(x, a, 20)
        .vote(a, m, 100)
        .vote(b, m, 200);
    // when: tallied
    let ledger = s.ledger();
    // then: A is out at 20, its vote against M is void, and M waits with the single vote of B
    assert_eq!(ledger.excluded, excluded(&[(a, by_votes(20, &[m, x]))]));
    assert_eq!(ledger.pending, BTreeMap::from([(m, waiting(&[b], 2))]));
}

#[test]
fn honest_second_vote_still_excludes_colluder_in_five() {
    // given: the same, and C votes against M at 300
    let Cast { m, a, b, c, x, .. } = cast();
    let s = members(b, &[a, c, m, x]);
    let s = s
        .vote(m, a, 10)
        .vote(x, a, 20)
        .vote(a, m, 100)
        .vote(b, m, 200)
        .vote(c, m, 300);
    // when: tallied
    let ledger = s.ledger();
    // then: M is excluded by B and C, in order of time
    assert_eq!(ledger.excluded.get(&m), Some(&by_votes(300, &[b, c])));
    assert!(ledger.pending.is_empty());
}

#[test]
fn stranger_cannot_vote_or_admit_in_three() {
    // given: me = A admits B and C; D admits itself and A and votes against A
    let Cast { a, b, c, d, .. } = cast();
    let s = members(a, &[b, c]).admit(d, d).admit(d, a).vote(d, a, 10);
    // when: tallied
    let ledger = s.ledger();
    // then: D is outside, and nobody has a vote against A
    assert_eq!(ledger.admitted, set(&[a, b, c]));
    assert!(ledger.pending.is_empty() && ledger.excluded.is_empty());
}

// --- helpers ---

/// M admits A, B, C, D and E (need 2); B and D have two waiting votes each from A and C. Natural input
/// never leaves two candidates (the pass excludes at the moment the threshold is reached), so the
/// state is set up by hand through the pending list of the pass: (time, voter, subject) and voter.
fn pass_with_two_candidates() -> Pass {
    let Cast { m, a, b, c, d, x } = cast();
    let input = members(m, &[a, b, c, d, x]).input;
    let mut pass = Pass::new(&input);
    for (subject, first_ts) in [(b, 10), (d, 20)] {
        let votes = [(first_ts, a), (first_ts + 1, c)];
        let rows = votes
            .iter()
            .map(|&(ts, voter)| ((ts, voter, subject), voter));
        pass.pending.insert(subject, rows.collect());
    }
    pass
}

/// The ten-node scenario as plain tuples, so the lists can be shuffled: node `i` votes against the
/// next node (and later against the one after), repeated pairs, three votes share a timestamp.
type Rows = (Vec<(PeerId, PeerId)>, Vec<(PeerId, PeerId, u64)>);

fn crowd() -> Rows {
    let node = |i: usize| pid(1 + (i % 10) as u8);
    let edges = [
        (0, 1),
        (0, 2),
        (1, 3),
        (2, 4),
        (3, 5),
        (4, 6),
        (5, 7),
        (6, 8),
        (7, 9),
        (1, 4),
        (2, 5),
    ];
    let edges = [&edges[..], &[(0, 3), (0, 6), (0, 7), (1, 8), (2, 9)]].concat();
    let admits = edges.iter().map(|&(x, y)| (node(x), node(y))).collect();
    let votes = (0..30).map(|i| crowd_vote(i, node)).collect();
    (admits, votes)
}

fn crowd_vote(i: usize, node: impl Fn(usize) -> PeerId) -> (PeerId, PeerId, u64) {
    let target = (i + i / 10) % 10;
    let target = if target == i % 10 { target + 1 } else { target };
    (node(i % 10), node(target), 10 + (i as u64 / 3) * 5)
}

fn crowd_scene(admits: &[(PeerId, PeerId)], votes: &[(PeerId, PeerId, u64)]) -> Scene {
    let first = pid(1);
    let s = admits.iter().fold(scene(first), |s, &(x, y)| s.admit(x, y));
    votes
        .iter()
        .fold(s, |s, &(voter, subject, ts)| s.vote(voter, subject, ts))
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
