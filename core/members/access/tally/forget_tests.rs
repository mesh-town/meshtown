//! Tests of the forget pass inside the tally (`pass.rs`, `forget.rs`), part one: who is forgotten,
//! who returns and what a returning admit spends. The forgotten author's past actions, exclusion and
//! order independence: `forget_order_tests.rs`. Thresholds and the cascade: `forget_vote_tests.rs`.
//! In `net(n)` the first node is the own node.

use super::support::*;
use super::*;

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
