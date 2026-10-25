//! Tests of the membership list tally (`pass.rs`, `votes.rs`): who is listed, how votes exclude, how the
//! need changes with the network size. Cascades, determinism and attack scenarios live in
//! `cascade_tests.rs`. Nodes: `m` is the own node, `a`..`d` are members, `x` is a stranger.

use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;
use std::collections::{BTreeMap, BTreeSet};

// --- admits ---

#[test]
fn me_alone_is_member_and_need_is_one() {
    // given: no records at all
    let Cast { m, .. } = cast();
    // when: tallied
    let ledger = scene(m).ledger();
    // then: only me is listed, nobody is excluded or pending, one vote suffices
    assert_eq!(ledger.admitted, set(&[m]));
    assert!(ledger.excluded.is_empty() && ledger.pending.is_empty());
    assert_eq!(ledger.need, 1);
    assert!(ledger.is_member(&m));
}

#[test]
fn one_vote_admits_two_newcomers_at_once() {
    // given: M and A invited each other, then A invites X and Y at once
    let Cast { m, a, x, d: y, .. } = cast();
    let s = scene(m)
        .admit_via(m, a, Via::Invite)
        .admit_via(a, m, Via::Invite);
    let s = s.admit_via(a, x, Via::Invite).admit_via(a, y, Via::Invite);
    // when: tallied
    let ledger = s.ledger();
    // then: the old pair is not a request; the single vote of A admits both newcomers
    assert!(ledger.admissions.is_empty());
    assert!(ledger.is_member(&x) && ledger.is_member(&y));
}

#[test]
fn stranger_self_admit_is_ignored() {
    // given: X admits itself and A, though nobody listed X
    let Cast { m, a, x, .. } = cast();
    let s = scene(m).admit(x, x).admit(x, a);
    // when: tallied
    let ledger = s.ledger();
    // then: neither X nor A is listed
    assert_eq!(ledger.admitted, set(&[m]));
    assert!(!ledger.is_member(&x) && !ledger.is_member(&a));
}

#[test]
fn admit_by_unlisted_author_is_ignored() {
    // given: M admits A; X, whom nobody admitted, admits B
    let Cast { m, a, b, x, .. } = cast();
    let s = scene(m).admit(m, a).admit(x, b);
    // when: tallied
    let ledger = s.ledger();
    // then: B is not a member
    assert_eq!(ledger.admitted, set(&[m, a]));
    assert!(!ledger.is_member(&b));
}

// --- votes ---

#[test]
fn two_members_one_vote_excludes() {
    // given: M and A, and M votes against A
    let Cast { m, a, .. } = cast();
    let s = members(m, &[a]).vote(m, a, 10);
    // when: tallied
    let ledger = s.ledger();
    // then: A is excluded by votes, the only voter is M; M stays a member
    assert_eq!(ledger.excluded, excluded(&[(a, by_votes(10, &[m]))]));
    assert!(ledger.is_member(&m) && !ledger.is_member(&a));
    assert!(ledger.pending.is_empty());
}

#[test]
fn three_members_one_vote_is_pending_with_proposer() {
    // given: M, A, B, and A votes against B
    let Cast { m, a, b, .. } = cast();
    let s = members(m, &[a, b]).vote(a, b, 10);
    // when: tallied
    let ledger = s.ledger();
    // then: the vote waits for a second one, A is the proposer, B is still a member
    assert_eq!(ledger.pending, BTreeMap::from([(b, waiting(&[a], 2))]));
    assert_eq!(ledger.need, 2);
    assert!(ledger.is_member(&b) && ledger.excluded.is_empty());
}

#[test]
fn three_members_second_vote_excludes() {
    // given: A votes against B, and M votes against B later
    let Cast { m, a, b, .. } = cast();
    let s = members(m, &[a, b]).vote(a, b, 10).vote(m, b, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: B is excluded at the time of the second vote, voters in time order
    assert_eq!(ledger.excluded, excluded(&[(b, by_votes(20, &[a, m]))]));
    assert!(ledger.pending.is_empty() && !ledger.is_member(&b));
}

#[test]
fn self_vote_excludes_at_any_size() {
    // given: five members, and A votes against itself
    let Cast { m, a, b, c, d, .. } = cast();
    let s = members(m, &[a, b, c, d]).vote(a, a, 5);
    // when: tallied
    let ledger = s.ledger();
    // then: A is excluded as having left, with itself as the only voter; the rest stay
    assert_eq!(ledger.excluded, excluded(&[(a, left(5, a))]));
    assert!([m, b, c, d].iter().all(|p| ledger.is_member(p)));
}

#[test]
fn exclusion_beats_admit_written_later() {
    // given: A is admitted by M, B and C, and B and C vote against A
    let Cast { m, a, b, c, .. } = cast();
    let s = members(m, &[a, b, c]).admit(b, a).admit(c, a);
    let s = s.vote(b, a, 10).vote(c, a, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: A is excluded however many admit it
    assert_eq!(ledger.excluded, excluded(&[(a, by_votes(20, &[b, c]))]));
    assert!(!ledger.is_member(&a));
}

#[test]
fn withdrawn_vote_returns_member() {
    // given: four members; two inputs, one with votes of B and C against A, one without C's
    let Cast { m, a, b, c, .. } = cast();
    let base = || members(m, &[a, b, c]).vote(b, a, 10);
    // when: both are tallied
    let with_second = base().vote(c, a, 20).ledger();
    let withdrawn = base().ledger();
    // then: with two votes A is out; without the second A is a member, one vote waits
    assert!(!with_second.is_member(&a));
    assert!(withdrawn.is_member(&a));
    assert_eq!(withdrawn.pending, BTreeMap::from([(a, waiting(&[b], 2))]));
}

#[test]
fn vote_from_unlisted_author_is_ignored() {
    // given: three members, and a stranger votes against A
    let Cast { m, a, b, x, .. } = cast();
    let s = members(m, &[a, b]).vote(x, a, 10);
    // when: tallied
    let ledger = s.ledger();
    // then: the vote counts for nothing
    assert!(ledger.pending.is_empty() && ledger.excluded.is_empty());
    assert!(ledger.is_member(&a));
}

#[test]
fn vote_after_voter_excluded_is_ignored() {
    // given: A leaves at 10, then votes against B at 20 (with A gone one vote would suffice)
    let Cast { m, a, b, .. } = cast();
    let s = members(m, &[a, b]).vote(a, a, 10).vote(a, b, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: B has no votes and is a member; only A is excluded
    assert!(ledger.is_member(&b) && ledger.pending.is_empty());
    assert_eq!(ledger.excluded, excluded(&[(a, left(10, a))]));
}

#[test]
fn mutual_votes_in_pair_first_wins() {
    // given: M and A; A votes against M at 10, M against A at 20
    let Cast { m, a, .. } = cast();
    let s = members(m, &[a]).vote(a, m, 10).vote(m, a, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: M is excluded by the earlier vote; the later vote of the excluded M does not count
    assert_eq!(ledger.excluded, excluded(&[(m, by_votes(10, &[a]))]));
    assert!(ledger.is_member(&a));
}

#[test]
fn pending_votes_of_excluded_voter_drop() {
    // given: four members; A votes against C at 10, then M and B exclude A at 20 and 30
    let Cast { m, a, b, c, .. } = cast();
    let s = members(m, &[a, b, c])
        .vote(a, c, 10)
        .vote(m, a, 20)
        .vote(b, a, 30);
    // when: tallied
    let ledger = s.ledger();
    // then: A is out and takes its waiting vote against C with it
    assert_eq!(ledger.excluded, excluded(&[(a, by_votes(30, &[m, b]))]));
    assert!(
        ledger.pending.is_empty(),
        "no vote on C is left: {:?}",
        ledger.pending
    );
    assert!(ledger.is_member(&c));
}

#[test]
fn need_drops_to_one_when_network_shrinks_to_two() {
    // given: M, A, B; A votes against B at 10 (waits), M leaves at 20
    let Cast { m, a, b, .. } = cast();
    let s = members(m, &[a, b]).vote(a, b, 10).vote(m, m, 20);
    // when: tallied
    let ledger = s.ledger();
    // then: two active nodes need one vote, so the waiting vote excludes B at the moment M left
    let want = excluded(&[(m, left(20, m)), (b, by_votes(20, &[a]))]);
    assert_eq!(ledger.excluded, want);
    assert_eq!(ledger.need, 1);
}

#[test]
fn mutual_votes_before_shrink_exclude_the_earlier_target() {
    // given: M, A, B; A votes against B at 10, B against A at 20, then M leaves at 30
    let Cast { m, a, b, .. } = cast();
    let s = members(m, &[a, b])
        .vote(a, b, 10)
        .vote(b, a, 20)
        .vote(m, m, 30);
    // when: tallied
    let ledger = s.ledger();
    // then: B, who was voted against first, goes; B's vote against A goes with it
    let want = excluded(&[(m, left(30, m)), (b, by_votes(30, &[a]))]);
    assert_eq!(ledger.excluded, want);
    assert!(ledger.is_member(&a) && ledger.pending.is_empty());
}

// --- scene and helpers (shared with `cascade_tests.rs`) ---

/// The cast: `m` is the own node, `a`..`d` are potential members, `x` is a stranger.
pub(super) struct Cast {
    pub(super) m: PeerId,
    pub(super) a: PeerId,
    pub(super) b: PeerId,
    pub(super) c: PeerId,
    pub(super) d: PeerId,
    pub(super) x: PeerId,
}

pub(super) fn cast() -> Cast {
    Cast {
        m: pid(1),
        a: pid(2),
        b: pid(3),
        c: pid(4),
        d: pid(5),
        x: pid(9),
    }
}

/// Input of the tally built record by record.
pub(super) struct Scene {
    pub(super) input: TallyInput,
}

pub(super) fn scene(me: PeerId) -> Scene {
    Scene {
        input: TallyInput {
            me,
            admits: vec![],
            votes: vec![],
            forgets: Vec::new(),
            rules: Rules::Peer,
            powers: vec![],
        },
    }
}

/// `me` admits every one of `others`.
pub(super) fn members(me: PeerId, others: &[PeerId]) -> Scene {
    others
        .iter()
        .fold(scene(me), |s, other| s.admit(me, *other))
}

impl Scene {
    pub(super) fn admit(self, author: PeerId, subject: PeerId) -> Scene {
        self.admit_via(author, subject, Via::None)
    }

    pub(super) fn admit_via(mut self, author: PeerId, subject: PeerId, via: Via) -> Scene {
        self.input.admits.push(AdmitIn {
            author,
            subject,
            via,
            made_unix: 0,
        });
        self
    }

    pub(super) fn vote(mut self, voter: PeerId, subject: PeerId, ts_micros: u64) -> Scene {
        self.input.votes.push(vote_in(voter, subject, ts_micros));
        self
    }

    pub(super) fn ledger(&self) -> Ledger {
        tally(&self.input)
    }
}

fn vote_in(voter: PeerId, subject: PeerId, ts_micros: u64) -> VoteIn {
    VoteIn {
        voter,
        subject,
        ts_micros,
        at_moment: false,
    }
}

pub(super) fn set(ids: &[PeerId]) -> BTreeSet<PeerId> {
    ids.iter().copied().collect()
}

pub(super) fn excluded(rows: &[(PeerId, Exclusion)]) -> BTreeMap<PeerId, Exclusion> {
    rows.iter().cloned().collect()
}

pub(super) fn by_votes(at_micros: u64, voters: &[PeerId]) -> Exclusion {
    Exclusion {
        how: How::Votes,
        at_micros,
        voters: voters.to_vec(),
    }
}

pub(super) fn left(at_micros: u64, who: PeerId) -> Exclusion {
    Exclusion {
        how: How::Left,
        at_micros,
        voters: vec![who],
    }
}

pub(super) fn waiting(voters: &[PeerId], need: u8) -> PendingVote {
    PendingVote {
        voters: voters.to_vec(),
        need,
    }
}
