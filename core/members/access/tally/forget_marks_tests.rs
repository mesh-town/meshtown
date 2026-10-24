//! Tests of the forget pass inside the tally (`pass.rs`, `forget.rs`), part four: the marks taken from the
//! raw records, whose votes count and a batch of votes. Thresholds and the cascade:
//! `forget_vote_tests.rs`.

use super::support::*;
use super::*;
use crate::transport::net::contract::pid;

// --- marks taken from the raw records ---

mod marks {
    use super::*;

    #[test]
    fn forget_mark_counts_uncounted_votes() {
        // given: three members; A and C exclude B (20 s, 30 s)
        let mut w = setup(3);
        w.exclude(A, B, 20).exclude(C, B, 30);
        // when: B's vote to forget C (300) is skipped as the vote of an excluded author
        let ledger = w.forget(B, C, 300).ledger();
        // then: it forgets nobody, yet its time is the mark
        assert!(ledger.excluded.contains_key(&w.id(B)));
        assert!(ledger.is_member(&w.id(C)) && !ledger.forgets.contains_key(&w.id(C)));
        assert_eq!(ledger.forget_mark(&w.id(C)), Some(300));
    }

    #[test]
    fn forget_mark_is_the_latest_vote_whatever_the_order() {
        // given: four members; B voted to forget D at 300 and A at 200, listed in that order
        let mut w = setup(4);
        w.forget(B, D, 300).forget(A, D, 200);
        // when: tallied
        let ledger = w.ledger();
        // then: the mark is the later time; a node without votes has no mark
        assert_eq!(ledger.forget_mark(&w.id(D)), Some(300));
        assert_eq!(ledger.forget_mark(&w.id(C)), None);
    }

    #[test]
    fn last_admit_unix_counts_dead_admits() {
        // given: three members; A and C forgot B (100, 110)
        let newcomer = pid(90);
        let mut w = setup(3);
        w.forget(A, B, 100).forget(C, B, 110);
        // when: B admits a newcomer at 150, after it was forgotten
        let ledger = w.admit(B, newcomer, Via::Invite, 150).ledger();
        // then: the admission is dead, the newcomer is no member, but the mark keeps its time
        assert!(!ledger.is_member(&newcomer));
        assert_eq!(ledger.last_admit_unix(&newcomer), Some(150));
    }
}

// --- whose votes count ---

mod voters {
    use super::*;

    #[test]
    fn vote_of_forgotten_author_after_forget_ignored() {
        // given: four members; A and C forgot B (100, 110)
        let mut w = setup(4);
        w.forget(A, B, 100).forget(C, B, 110);
        // when: B votes to forget D (200) and A does (300)
        let ledger = w.forget(B, D, 200).forget(A, D, 300).ledger();
        // then: B's vote is skipped: D waits on A alone
        assert!(ledger.is_forgotten(&w.id(B)));
        w.assert_round(&ledger, D, &[A], None);
        assert!(ledger.is_member(&w.id(D)));
    }

    #[test]
    fn vote_before_author_forgotten_counts() {
        // given: four members; B and A forgot D (50, 60)
        let mut w = setup(4);
        w.forget(B, D, 50).forget(A, D, 60);
        // when: A and C forget B later (100, 110)
        let ledger = w.forget(A, B, 100).forget(C, B, 110).ledger();
        // then: both are forgotten; B's earlier vote stays in the round of D
        w.assert_round(&ledger, D, &[B, A], Some(60));
        assert_eq!(ledger.forgotten_at(&w.id(B)), Some(110));
    }

    #[test]
    fn excluded_subject_has_no_round() {
        // given: three members; A and C vote to exclude B (20 s, 30 s)
        let mut w = setup(3);
        w.exclude(A, B, 20).exclude(C, B, 30);
        // when: A votes to forget B (100)
        let ledger = w.forget(A, B, 100).ledger();
        // then: B is excluded and has no forget round
        assert!(ledger.excluded.contains_key(&w.id(B)));
        assert!(!ledger.forgets.contains_key(&w.id(B)));
    }

    #[test]
    fn exclusion_votes_need_counts_without_forgotten() {
        // given: four members; A and B forgot D, three are active
        let mut w = setup(4);
        w.forget(A, D, 100).forget(B, D, 110);
        // when: A votes to exclude B
        let ledger = w.exclude(A, B, 200).ledger();
        // then: the vote waits for a second one
        let want = PendingVote {
            voters: vec![w.id(A)],
            need: 2,
        };
        assert!(ledger.is_forgotten(&w.id(D)));
        assert_eq!(ledger.pending.get(&w.id(B)), Some(&want));
        assert!(ledger.excluded.is_empty());
    }

    #[test]
    fn exclusion_needs_one_vote_when_forgotten_leave_two_active() {
        // given: three members; A and B forgot C, two are active
        let mut w = setup(3);
        w.forget(A, C, 100).forget(B, C, 110);
        // when: A votes to exclude B
        let ledger = w.exclude(A, B, 200).ledger();
        // then: one vote excludes
        assert!(ledger.excluded.contains_key(&w.id(B)));
    }
}

// --- a batch of votes ---

mod batch {
    use super::*;

    #[test]
    fn batch_of_votes_waits_each_for_second() {
        // given: six members; A votes to forget D, E and F (100..102)
        let mut w = setup(6);
        w.forget(A, D, 100).forget(A, E, 101).forget(A, F, 102);
        // when: tallied
        let ledger = w.ledger();
        // then: all three are members, each round holds A alone
        for who in [D, E, F] {
            assert!(ledger.is_member(&w.id(who)), "node {who} is a member");
            w.assert_round(&ledger, who, &[A], None);
        }
        // when: B votes for the same three (200..202)
        w.forget(B, D, 200).forget(B, E, 201).forget(B, F, 202);
        let ledger = w.ledger();
        // then: all three are forgotten, each at the time of B's vote
        for (who, at) in [(D, 200), (E, 201), (F, 202)] {
            w.assert_round(&ledger, who, &[A, B], Some(at));
        }
    }
}
