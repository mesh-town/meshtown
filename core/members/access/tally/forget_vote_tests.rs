//! Tests of the forget pass inside the tally (`pass.rs`, `forget.rs`), part three: the vote threshold,
//! the cascade after a forgetting, growth of the network, mutual and withdrawn votes. The marks taken
//! from the raw records and whose votes count: `forget_marks_tests.rs`. The nodes of `net(n)` are
//! named by index: `A` is the own node.

use super::support::*;
use super::*;
use crate::transport::net::contract::pid;

// --- threshold ---

mod threshold {
    use super::*;

    #[test]
    fn one_vote_forgets_when_two_active() {
        // given: two members, A and B
        let mut w = setup(2);
        // when: A votes to forget B
        let ledger = w.forget(A, B, 100).ledger();
        // then: B is forgotten at the time of the vote, A is the only voter, one vote was enough
        assert!(ledger.is_forgotten(&w.id(B)) && !ledger.is_member(&w.id(B)));
        w.assert_round(&ledger, B, &[A], Some(100));
        assert_eq!(ledger.need, 1);
    }

    #[test]
    fn one_vote_waits_when_three_active() {
        // given: three members
        let mut w = setup(3);
        // when: A votes to forget C
        let ledger = w.forget(A, C, 100).ledger();
        // then: C is still a member, the round waits with A as the proposer, two votes are needed
        assert!(ledger.is_member(&w.id(C)) && !ledger.is_forgotten(&w.id(C)));
        w.assert_round(&ledger, C, &[A], None);
        assert_eq!(ledger.need, 2);
    }

    #[test]
    fn second_vote_forgets_when_three_active() {
        // given: three members; A voted to forget C at 100
        let mut w = setup(3);
        w.forget(A, C, 100);
        // when: B votes to forget C as well, at 200
        let ledger = w.forget(B, C, 200).ledger();
        // then: C is forgotten at the time of the second vote, voters in time order
        assert_eq!(ledger.forgotten_at(&w.id(C)), Some(200));
        w.assert_round(&ledger, C, &[A, B], Some(200));
        assert!(ledger.is_member(&w.id(A)) && ledger.is_member(&w.id(B)));
    }
}

// --- cascade ---

mod cascade {
    use super::*;

    #[test]
    fn forgetting_drops_need_and_settles_waiting_round() {
        // given: three members; A waits on C (10) and on B (20)
        let mut w = setup(3);
        w.forget(A, C, 10).forget(A, B, 20);
        // when: C votes to forget B (30), the second vote that forgets B
        let ledger = w.forget(C, B, 30).ledger();
        // then: two are left, one vote is enough, so C is forgotten at the same moment by A's vote
        w.assert_round(&ledger, B, &[A, C], Some(30));
        w.assert_round(&ledger, C, &[A], Some(30));
        assert!(ledger.is_member(&w.id(A)));
    }

    #[test]
    fn forgetting_to_three_active_keeps_other_round_waiting() {
        // given: four members; A waits on C (20) and A voted to forget D (10)
        let mut w = setup(4);
        w.forget(A, D, 10).forget(A, C, 20);
        // when: B votes to forget D (30)
        let ledger = w.forget(B, D, 30).ledger();
        // then: D is forgotten, three are left and two votes are still needed: C waits on A alone
        assert_eq!(ledger.forgotten_at(&w.id(D)), Some(30));
        w.assert_round(&ledger, C, &[A], None);
        assert!(ledger.is_member(&w.id(C)));
        assert_eq!(ledger.need, 2);
    }

    #[test]
    fn exclusion_lowering_need_completes_pending_removal() {
        // given: three members; A waits on C
        let mut w = setup(3);
        w.forget(A, C, 100);
        // when: A and C vote to exclude B (20 s, 30 s), so B is excluded
        let ledger = w.exclude(A, B, 20).exclude(C, B, 30).ledger();
        // then: two are left, A's single vote forgets C, and C's earlier exclusion vote still stands
        assert!(ledger.excluded.contains_key(&w.id(B)));
        assert_eq!(ledger.forgotten_at(&w.id(C)), Some(100));
    }
}

// --- growth of the network ---

mod growth {
    use super::*;

    #[test]
    fn newcomer_reopens_single_vote_removal() {
        // given: two members; A voted to forget B at 100, so B was forgotten by one vote
        let mut w = setup(2);
        w.forget(A, B, 100);
        assert!(w.ledger().is_forgotten(&w.id(B)));
        // when: A admits a newcomer at 150
        let ledger = w.admit(A, pid(90), Via::Invite, 150).ledger();
        // then: three are active, B is a member again, the round is [A] and a second vote is needed
        assert!(ledger.is_member(&w.id(B)) && ledger.is_member(&pid(90)));
        w.assert_round(&ledger, B, &[A], None);
        assert_eq!(ledger.need, 2);
    }
}

// --- mutual votes ---

mod mutual_votes {
    use super::*;

    #[test]
    fn mutual_votes_forget_only_the_earlier_target() {
        // given: two members; A votes to forget B at 100, B votes to forget A at 200
        let mut w = setup(2);
        w.forget(A, B, 100).forget(B, A, 200);
        // when: tallied
        let ledger = w.ledger();
        // then: B is forgotten, its vote is skipped, A stays a member and has no round
        assert!(ledger.is_forgotten(&w.id(B)));
        assert!(ledger.is_member(&w.id(A)) && !ledger.forgets.contains_key(&w.id(A)));
    }

    #[test]
    fn forgotten_author_waiting_votes_dropped() {
        // given: three members; B waits on A (50), then A (100) and C (120) vote to forget B
        let mut w = setup(3);
        w.forget(B, A, 50).forget(A, B, 100).forget(C, B, 120);
        // when: tallied
        let ledger = w.ledger();
        // then: B is forgotten, its waiting vote is gone with the round on A, A is a member
        w.assert_round(&ledger, B, &[A, C], Some(120));
        assert!(!ledger.forgets.contains_key(&w.id(A)));
        assert!(ledger.is_member(&w.id(A)));
    }
}

// --- withdrawn votes ---

mod withdrawn_votes {
    use super::*;

    #[test]
    fn withdrawn_second_vote_returns_member() {
        // given: three members; A and B forgot C
        let mut w = setup(3);
        w.forget(A, C, 100).forget(B, C, 110);
        assert!(w.ledger().is_forgotten(&w.id(C)), "two votes forget C");
        // when: B withdraws the vote (the tombstone leaves the input)
        let ledger = w.without_forget(B, C).ledger();
        // then: C is a member again and the round is [A]
        assert!(ledger.is_member(&w.id(C)));
        w.assert_round(&ledger, C, &[A], None);
    }
}
