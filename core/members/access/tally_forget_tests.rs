//! Tests of the forget pass inside the tally (`tally.rs`, `forget.rs`), part one: who is forgotten,
//! who returns, what a returning admit spends, what the forgotten author did before, and order
//! independence. Thresholds and the cascade: `tally_forget_vote_tests.rs`. In `net(n)` the first
//! node is the own node.

use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

mod forgetting {
    use super::*;

    #[test]
    fn forgotten_member_is_not_a_member() {
        // given: A and B are the only members, A votes to forget B at 100
        let (a, p, admits) = net(2);
        let b = p[1];
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, b, 100)]);
        // then: B is forgotten at 100 and no longer a member, A is; B stays in the admitted set
        assert_eq!(ledger.forgotten_at(&b), Some(100));
        assert!(!ledger.is_member(&b) && ledger.is_member(&a));
        assert!(
            ledger.admitted.contains(&b),
            "a forgotten node keeps its place in the admitted set"
        );
    }

    #[test]
    fn forget_by_excluded_author_is_ignored() {
        // given: A excluded B at 10 s, and B (already excluded) votes to forget A at 100
        let (a, p, admits) = net(2);
        let b = p[1];
        let votes = vec![vote_in(a, b, 10)];
        // when: tallied
        let ledger = run_votes(a, admits, vec![forget(b, a, 100)], votes);
        // then: the vote of the excluded author does not count
        assert!(ledger.excluded.contains_key(&b));
        assert!(ledger.is_member(&a) && ledger.forgets.is_empty());
    }

    #[test]
    fn forget_by_later_excluded_author_dropped() {
        // given: four members; B and A vote to forget D (100, 110), then A and C exclude B
        let (a, p, admits) = net(4);
        let (b, c, d) = (p[1], p[2], p[3]);
        let votes = vec![vote_in(a, b, 200), vote_in(c, b, 210)];
        let forgets = vec![forget(b, d, 100), forget(a, d, 110)];
        // when: tallied
        let ledger = run_votes(a, admits, forgets, votes);
        // then: B's vote is dropped from the start of the pass: D is a member, A's vote waits
        assert!(ledger.excluded.contains_key(&b));
        assert!(ledger.is_member(&d));
        assert_eq!(ledger.forgets.get(&d), Some(&round(&[a], None)));
    }

    #[test]
    fn self_forget_is_ignored() {
        // given: both members vote to forget themselves
        let (a, p, admits) = net(2);
        let b = p[1];
        let forgets = vec![forget(a, a, 100), forget(b, b, 100)];
        // when: tallied
        let ledger = run(a, admits, forgets);
        // then: nobody is forgotten and there are no rounds
        assert!(ledger.is_member(&a) && ledger.is_member(&b));
        assert!(ledger.forgets.is_empty());
    }
}

mod return_of_forgotten {
    use super::*;

    #[test]
    fn admit_after_forget_returns_member() {
        // given: A forgot B at 100, then A admits B again at 101
        let (a, p, admits) = net(2);
        let b = p[1];
        let admits = rewrite(admits, a, b, Via::Invite, 101);
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, b, 100)]);
        // then: B is a member again
        assert!(ledger.is_member(&b));
        assert!(!ledger.is_forgotten(&b));
    }

    #[test]
    fn admit_at_same_second_does_not_return() {
        // given: the admit of B is written in the same second as the vote against B
        let (a, p, admits) = net(2);
        let b = p[1];
        let admits = rewrite(admits, a, b, Via::Invite, 100);
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, b, 100)]);
        // then: the vote goes after the admit, B is forgotten
        assert_eq!(ledger.forgotten_at(&b), Some(100));
        assert!(!ledger.is_member(&b));
    }

    #[test]
    fn witness_after_forget_does_not_return() {
        // given: C forgotten by A and B (100); A and B then write witnesses of C (150)
        let (a, p, admits) = net(3);
        let (b, c) = (p[1], p[2]);
        let admits = rewrite(admits, a, c, Via::Witness, 150);
        let admits = rewrite(admits, b, c, Via::Witness, 150);
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, c, 90), forget(b, c, 100)]);
        // then: a witness does not return anybody
        assert_eq!(ledger.forgotten_at(&c), Some(100));
        assert!(!ledger.is_member(&c));
    }

    #[test]
    fn forgotten_pair_cannot_readmit_each_other() {
        // given: five members; A, B forgotten (100, 101), then admit each other (150)
        let (me, p, admits) = net(5);
        let (a, b, c) = (p[1], p[2], p[3]);
        let admits = rewrite(admits, a, b, Via::Invite, 150);
        let admits = rewrite(admits, b, a, Via::Invite, 150);
        let forgets = vec![
            forget(me, a, 90),
            forget(c, a, 100),
            forget(me, b, 91),
            forget(c, b, 101),
        ];
        // when: tallied
        let ledger = run(me, admits, forgets);
        // then: both stay forgotten: a forgotten author cannot return anyone
        assert!(ledger.is_forgotten(&a) && ledger.is_forgotten(&b));
        assert!(!ledger.is_member(&a) && !ledger.is_member(&b));
    }

    #[test]
    fn admit_between_votes_resets_round() {
        // given: four members; A votes against C at 100, A admits C again at 150, B votes at 200
        let (a, p, admits) = net(4);
        let (b, c) = (p[1], p[2]);
        let admits = rewrite(admits, a, c, Via::Invite, 150);
        let forgets = vec![forget(a, c, 100), forget(b, c, 200)];
        // when: tallied
        let ledger = run(a, admits, forgets);
        // then: the admit cleared the vote of A: C is a member, the round holds only B
        assert!(ledger.is_member(&c));
        assert_eq!(ledger.forgets.get(&c), Some(&round(&[b], None)));
        assert_eq!(ledger.forget_mark(&c), Some(200));
    }
}

mod returning_admit {
    use super::*;

    #[test]
    fn spends_round() {
        // given: three members; A and B forgot C (100, 110), A admits C again at 200
        let (a, p, admits) = net(3);
        let (b, c) = (p[1], p[2]);
        let admits = rewrite(admits, a, c, Via::Invite, 200);
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, c, 100), forget(b, c, 110)]);
        // then: C is a member and has no round left
        assert!(ledger.is_member(&c));
        assert!(
            !ledger.forgets.contains_key(&c),
            "spent votes are not a round"
        );
    }

    #[test]
    fn new_vote_after_return_opens_new_round() {
        // given: four members; B voted against C (110), A admitted C (200), A's latest vote is at 300
        let (a, p, admits) = net(4);
        let (b, c) = (p[1], p[2]);
        let admits = rewrite(admits, a, c, Via::Invite, 200);
        let forgets = vec![forget(b, c, 110), forget(a, c, 300)];
        // when: tallied
        let ledger = run(a, admits, forgets);
        // then: the vote of B was spent by the admit, the new round holds A alone and waits
        assert!(ledger.is_member(&c));
        assert_eq!(ledger.forgets.get(&c), Some(&round(&[a], None)));
    }
}

mod past_actions_of_forgotten {
    use super::*;

    #[test]
    fn invitee_of_forgotten_stays_member() {
        // given: A, B, C; B admitted D at 50; A and C forgot B (100, 110)
        let (a, p, admits) = net(3);
        let (b, c) = (p[1], p[2]);
        let d = pid(40);
        let admits = rewrite(admits, b, d, Via::Invite, 50);
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, b, 100), forget(c, b, 110)]);
        // then: B is forgotten, but D, admitted before that, is a member
        assert!(ledger.is_forgotten(&b));
        assert!(
            ledger.is_member(&d),
            "an admit made before the forgetting stays alive"
        );
    }

    #[test]
    fn admit_by_forgotten_after_forget_does_not_count() {
        // given: A, B, C; A and C forgot B (90, 100); B admits E at 150
        let (a, p, admits) = net(3);
        let (b, c) = (p[1], p[2]);
        let e = pid(41);
        let admits = rewrite(admits, b, e, Via::Invite, 150);
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, b, 90), forget(c, b, 100)]);
        // then: the admit of a forgotten author is dead
        assert!(ledger.is_forgotten(&b));
        assert!(!ledger.admitted.contains(&e) && !ledger.is_member(&e));
    }

    #[test]
    fn vote_of_forgotten_before_forget_keeps_exclusion() {
        // given: four members; A and B vote against X (10 s, 20 s), later M and B forget A
        let (m, p, admits) = net(4);
        let (a, b, x) = (p[1], p[2], p[3]);
        let votes = vec![vote_in(a, x, 10), vote_in(b, x, 20)];
        let forgets = vec![forget(m, a, 90), forget(b, a, 100)];
        // when: tallied
        let ledger = run_votes(m, admits, forgets, votes);
        // then: A is forgotten, but its vote cast before that still excludes X
        assert!(ledger.is_forgotten(&a));
        assert!(ledger.excluded.contains_key(&x));
        assert_eq!(ledger.excluded[&x].voters, vec![a, b]);
    }

    #[test]
    fn forgotten_subject_can_be_excluded() {
        // given: M, A, C; M and A forgot C (100, 110); then M and A vote to exclude C
        let (m, p, admits) = net(3);
        let (a, c) = (p[1], p[2]);
        let votes = vec![vote_in(m, c, 200), vote_in(a, c, 210)];
        let forgets = vec![forget(m, c, 100), forget(a, c, 110)];
        // when: tallied
        let ledger = run_votes(m, admits, forgets, votes);
        // then: C is excluded
        assert!(ledger.excluded.contains_key(&c));
        assert!(!ledger.is_member(&c));
    }
}

mod excluded_node {
    use super::*;

    #[test]
    fn stays_excluded_after_forget_and_admit() {
        // given: M and A exclude X (10 s, 20 s), forget it (100, 110) and M admits it at 200
        let (m, p, admits) = net(3);
        let (a, x) = (p[1], p[2]);
        let admits = rewrite(admits, m, x, Via::Invite, 200);
        let votes = vec![vote_in(m, x, 10), vote_in(a, x, 20)];
        let forgets = vec![forget(m, x, 100), forget(a, x, 110)];
        // when: tallied
        let ledger = run_votes(m, admits, forgets, votes);
        // then: X is still excluded, not a member and has no round
        assert!(ledger.excluded.contains_key(&x));
        assert!(!ledger.is_member(&x) && !ledger.forgets.contains_key(&x));
    }
}

mod properties {
    use super::*;

    #[test]
    fn entry_order_does_not_change_ledger() {
        // given: C forgotten, E admitted by C before that, a waiting round on A, a pending exclusion
        let (a, p, admits) = net(4);
        let (b, c, d) = (p[1], p[2], p[3]);
        let e = pid(42);
        let admits = rewrite(admits, c, e, Via::Invite, 50);
        let forgets = vec![forget(a, c, 100), forget(b, c, 110), forget(b, a, 120)];
        let votes = vec![vote_in(a, d, 7)];
        let base = run_votes(a, admits.clone(), forgets.clone(), votes.clone());
        assert!(
            base.is_forgotten(&c) && base.is_member(&e),
            "the input must exercise the pass"
        );
        assert_eq!(base.forgets.get(&a), Some(&round(&[b], None)));
        // when: the same records come in other orders; then: the ledger is the same
        let reversed = run_votes(a, rev(&admits), rev(&forgets), rev(&votes));
        assert_eq!(reversed, base);
        let left = run_votes(a, rot(&admits, 1), rot(&forgets, 1), rot(&votes, 1));
        assert_eq!(left, base);
        assert_eq!(run_votes(a, rot(&admits, 3), rot(&forgets, 2), votes), base);
    }

    #[test]
    fn forgetting_lowers_need() {
        // given: four members; A and B forgot C (100, 110)
        let (a, p, admits) = net(4);
        let (b, c, d) = (p[1], p[2], p[3]);
        let mut forgets = vec![forget(a, c, 100), forget(b, c, 110)];
        // when: tallied
        let after_one = run(a, admits.clone(), forgets.clone());
        // then: three are left, two votes are needed; A and D then forget B (120, 130)
        assert!(after_one.is_forgotten(&c));
        assert_eq!(after_one.need, 2);
        forgets.extend([forget(a, b, 120), forget(d, b, 130)]);
        let after_two = run(a, admits, forgets);
        // then: two are left, one vote is enough
        assert!(after_two.is_forgotten(&b));
        assert_eq!(after_two.need, 1);
    }
}

// --- helpers (`forget` and `net` are shared with `tally_forget_vote_tests.rs`) ---

pub(super) fn forget(author: PeerId, subject: PeerId, made_unix: u64) -> ForgetIn {
    ForgetIn {
        author,
        subject,
        made_unix,
    }
}

/// A network of `n` active members: `peers[0]` is the own node `me`, and `me` admits all the
/// others (`Via::Invite`, `made_unix` 0). Returns `me`, all `n` members and the admit records.
pub(super) fn net(n: usize) -> (PeerId, Vec<PeerId>, Vec<AdmitIn>) {
    let peers: Vec<PeerId> = (1..=n as u8).map(pid).collect();
    let me = peers[0];
    let admits = peers[1..]
        .iter()
        .map(|p| admit_at(me, *p, Via::Invite, 0))
        .collect();
    (me, peers, admits)
}

pub(super) fn admit_at(author: PeerId, subject: PeerId, via: Via, made_unix: u64) -> AdmitIn {
    AdmitIn {
        author,
        subject,
        via,
        made_unix,
    }
}

/// Rewrites the record of the pair (`author`, `subject`), as the replica does: one record per pair.
pub(super) fn rewrite(
    admits: Vec<AdmitIn>,
    author: PeerId,
    subject: PeerId,
    via: Via,
    made_unix: u64,
) -> Vec<AdmitIn> {
    let mut rest: Vec<AdmitIn> = admits
        .into_iter()
        .filter(|a| !(a.author == author && a.subject == subject))
        .collect();
    rest.push(admit_at(author, subject, via, made_unix));
    rest
}

pub(super) fn vote_in(voter: PeerId, subject: PeerId, secs: u64) -> VoteIn {
    VoteIn {
        voter,
        subject,
        ts_micros: secs * 1_000_000,
    }
}

pub(super) fn round(voters: &[PeerId], at_unix: Option<u64>) -> ForgetRound {
    ForgetRound {
        voters: voters.to_vec(),
        at_unix,
    }
}

pub(super) fn run(me: PeerId, admits: Vec<AdmitIn>, forgets: Vec<ForgetIn>) -> Ledger {
    run_votes(me, admits, forgets, Vec::new())
}

pub(super) fn run_votes(
    me: PeerId,
    admits: Vec<AdmitIn>,
    forgets: Vec<ForgetIn>,
    votes: Vec<VoteIn>,
) -> Ledger {
    tally(&TallyInput {
        me,
        admits,
        votes,
        forgets,
    })
}

fn rev<T: Clone>(items: &[T]) -> Vec<T> {
    items.iter().rev().cloned().collect()
}

fn rot<T: Clone>(items: &[T], by: usize) -> Vec<T> {
    let mut out = items.to_vec();
    out.rotate_left(by % items.len().max(1));
    out
}
