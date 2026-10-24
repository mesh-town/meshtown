//! Snapshots of the tally result (`Ledger`) for forget scenarios: thresholds, return of a forgotten node,
//! votes of a forgotten node and the cascade over its admissions. The input is built with `setup(n)`
//! (the nodes `A`.. are named by index) and the whole `Debug` text of the ledger is compared.

use super::characterization_tests::shown;
use super::support::*;
use super::*;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

mod threshold {
    use super::*;

    #[test]
    fn one_vote_forgets_when_two_are_active() {
        // given: two members; A votes to forget B at 100
        let mut w = setup(2);
        // when: tallied
        let ledger = w.forget(A, B, 100).ledger();
        // then: B is forgotten at once, the forget threshold is one
        assert_eq!(
            shown(&ledger, &lettered(&w, 2)),
            concat!(
                "Ledger { admitted: {B, A}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {B: ForgetRound { voters: [A], at_unix: Some(100) }}, ",
                "admit_unix: {B: 0}, ",
                "forget_marks: {B: 100}, ",
                "forget_need: 1 }",
            )
        );
    }

    #[test]
    fn one_vote_waits_when_three_are_active() {
        // given: three members; A votes to forget C at 100
        let mut w = setup(3);
        // when: tallied
        let ledger = w.forget(A, C, 100).ledger();
        // then: the round waits with A as the proposer, the forget threshold is two
        assert_eq!(
            shown(&ledger, &lettered(&w, 3)),
            concat!(
                "Ledger { admitted: {B, A, C}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {C: ForgetRound { voters: [A], at_unix: None }}, ",
                "admit_unix: {B: 0, C: 0}, ",
                "forget_marks: {C: 100}, ",
                "forget_need: 2 }",
            )
        );
    }

    #[test]
    fn second_vote_forgets_when_three_are_active() {
        // given: three members; A and B vote to forget C at 100 and 200
        let mut w = setup(3);
        // when: tallied
        let ledger = w.forget(A, C, 100).forget(B, C, 200).ledger();
        // then: C is forgotten at the second vote, two are left and one vote suffices
        assert_eq!(
            shown(&ledger, &lettered(&w, 3)),
            concat!(
                "Ledger { admitted: {B, A, C}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {C: ForgetRound { voters: [A, B], at_unix: Some(200) }}, ",
                "admit_unix: {B: 0, C: 0}, ",
                "forget_marks: {C: 200}, ",
                "forget_need: 1 }",
            )
        );
    }

    #[test]
    fn a_newcomer_raises_the_forget_threshold() {
        // given: two members; A admits E at 50, then votes to forget B at 100
        let e = pid(90);
        let mut w = setup(2);
        // when: tallied
        let ledger = w.admit(A, e, Via::Invite, 50).forget(A, B, 100).ledger();
        // then: three are active, so B waits for a second vote
        let mut names = lettered(&w, 2);
        names.push(("E", e));
        assert_eq!(
            shown(&ledger, &names),
            concat!(
                "Ledger { admitted: {E, B, A}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {B: ForgetRound { voters: [A], at_unix: None }}, ",
                "admit_unix: {E: 50, B: 0}, ",
                "forget_marks: {B: 100}, ",
                "forget_need: 2 }",
            )
        );
    }

    #[test]
    fn forgetting_down_to_three_keeps_the_second_vote_needed() {
        // given: four members; A waits on C (10), then A and B forget D (20, 30)
        let mut w = setup(4);
        w.forget(A, C, 10).forget(A, D, 20).forget(B, D, 30);
        // when: tallied
        let ledger = w.ledger();
        // then: D is forgotten, three are left and C still waits on A alone
        assert_eq!(shown(&ledger, &lettered(&w, 4)), concat!(
                "Ledger { admitted: {B, A, D, C}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {D: ForgetRound { voters: [A, B], at_unix: Some(30) }, C: ForgetRound { voters: [A], at_unix: None }}, ",
                "admit_unix: {B: 0, D: 0, C: 0}, ",
                "forget_marks: {D: 30, C: 10}, ",
                "forget_need: 2 }",
            ));
    }
}

mod return_of_forgotten {
    use super::*;

    #[test]
    fn admit_after_forget_returns_the_member() {
        // given: A forgot B at 100, then A admits B again at 101
        let (a, p, admits) = net(2);
        let admits = rewrite(admits, a, p[1], Via::Invite, 101);
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, p[1], 100)]);
        // then: B is a member again, the round is spent
        assert_eq!(
            shown(&ledger, &letters(&p)),
            concat!(
                "Ledger { admitted: {B, A}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {B: 101}, ",
                "forget_marks: {B: 100}, ",
                "forget_need: 1 }",
            )
        );
    }

    #[test]
    fn admit_in_the_same_second_does_not_return() {
        // given: the admit of B is written in the same second as the vote against B
        let (a, p, admits) = net(2);
        let admits = rewrite(admits, a, p[1], Via::Invite, 100);
        // when: tallied
        let ledger = run(a, admits, vec![forget(a, p[1], 100)]);
        // then: the vote goes after the admit, B is forgotten
        assert_eq!(
            shown(&ledger, &letters(&p)),
            concat!(
                "Ledger { admitted: {B, A}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {B: ForgetRound { voters: [A], at_unix: Some(100) }}, ",
                "admit_unix: {B: 100}, ",
                "forget_marks: {B: 100}, ",
                "forget_need: 1 }",
            )
        );
    }

    #[test]
    fn witness_after_forget_does_not_return() {
        // given: C is forgotten by A and B; A and B then write witnesses of C
        let (a, p, admits) = net(3);
        let admits = rewrite(admits, a, p[2], Via::Witness, 150);
        let admits = rewrite(admits, p[1], p[2], Via::Witness, 150);
        // when: tallied
        let ledger = run(
            a,
            admits,
            vec![forget(a, p[2], 90), forget(p[1], p[2], 100)],
        );
        // then: C stays forgotten
        assert_eq!(
            shown(&ledger, &letters(&p)),
            concat!(
                "Ledger { admitted: {B, A, C}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {C: ForgetRound { voters: [A, B], at_unix: Some(100) }}, ",
                "admit_unix: {B: 0}, ",
                "forget_marks: {C: 100}, ",
                "forget_need: 1 }",
            )
        );
    }

    #[test]
    fn forgotten_pair_cannot_readmit_each_other() {
        // given: five members; A and B are forgotten, then admit each other
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
        // then: both stay forgotten
        assert_eq!(shown(&ledger, &letters(&p)), concat!(
                "Ledger { admitted: {E, B, A, D, C}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {B: ForgetRound { voters: [A, D], at_unix: Some(100) }, C: ForgetRound { voters: [A, D], at_unix: Some(101) }}, ",
                "admit_unix: {E: 0, B: 150, D: 0, C: 150}, ",
                "forget_marks: {B: 100, C: 101}, ",
                "forget_need: 2 }",
            ));
    }
}

mod forgotten_node {
    use super::*;

    #[test]
    fn exclusion_vote_before_forgetting_counts_and_after_does_not() {
        // given: four members; D is forgotten by A and B at 100; D votes to exclude C at 50 and B at 150
        let mut w = setup(4);
        w.forget(A, D, 100)
            .forget(B, D, 100)
            .exclude(D, C, 50)
            .exclude(D, B, 150);
        // when: tallied
        let ledger = w.ledger();
        // then: only the vote against C is pending
        assert_eq!(
            shown(&ledger, &lettered(&w, 4)),
            concat!(
                "Ledger { admitted: {B, A, D, C}, ",
                "excluded: {}, ",
                "pending: {C: PendingVote { voters: [D], need: 2 }}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {D: ForgetRound { voters: [B, A], at_unix: Some(100) }}, ",
                "admit_unix: {B: 0, D: 0, C: 0}, ",
                "forget_marks: {D: 100}, ",
                "forget_need: 2 }",
            )
        );
    }

    #[test]
    fn admission_made_before_forgetting_still_counts() {
        // given: four members; B admits E at 50; A and C forget B at 100
        let e = pid(90);
        let mut w = setup(4);
        w.admit(B, e, Via::Invite, 50)
            .forget(A, B, 100)
            .forget(C, B, 100);
        // when: tallied
        let ledger = w.ledger();
        // then: B is forgotten, E, admitted before that, stays listed
        let mut names = lettered(&w, 4);
        names.push(("E", e));
        assert_eq!(
            shown(&ledger, &names),
            concat!(
                "Ledger { admitted: {E, B, A, D, C}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {B: ForgetRound { voters: [A, C], at_unix: Some(100) }}, ",
                "admit_unix: {E: 50, B: 0, D: 0, C: 0}, ",
                "forget_marks: {B: 100}, ",
                "forget_need: 2 }",
            )
        );
    }

    #[test]
    fn admission_made_after_forgetting_is_dead() {
        // given: four members; A and C forget B at 100; B admits E at 150
        let e = pid(90);
        let mut w = setup(4);
        w.forget(A, B, 100)
            .forget(C, B, 100)
            .admit(B, e, Via::Invite, 150);
        // when: tallied
        let ledger = w.ledger();
        // then: E, admitted by B alone, is neither listed nor a request
        let mut names = lettered(&w, 4);
        names.push(("E", e));
        assert_eq!(
            shown(&ledger, &names),
            concat!(
                "Ledger { admitted: {B, A, D, C}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {B: ForgetRound { voters: [A, C], at_unix: Some(100) }}, ",
                "admit_unix: {E: 150, B: 0, D: 0, C: 0}, ",
                "forget_marks: {B: 100}, ",
                "forget_need: 2 }",
            )
        );
    }

    #[test]
    fn second_admitter_keeps_the_subject_of_a_forgotten_author() {
        // given: the same, but D admitted E as well
        let e = pid(90);
        let mut w = setup(4);
        w.forget(A, B, 100).forget(C, B, 100);
        w.admit(B, e, Via::Invite, 150)
            .admit(D, e, Via::Invite, 160);
        // when: tallied
        let ledger = w.ledger();
        // then: E stays listed through D
        let mut names = lettered(&w, 4);
        names.push(("E", e));
        assert_eq!(
            shown(&ledger, &names),
            concat!(
                "Ledger { admitted: {E, B, A, D, C}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {B: ForgetRound { voters: [A, C], at_unix: Some(100) }}, ",
                "admit_unix: {E: 160, B: 0, D: 0, C: 0}, ",
                "forget_marks: {B: 100}, ",
                "forget_need: 2 }",
            )
        );
    }

    #[test]
    fn exclusion_lowering_the_threshold_completes_a_waiting_round() {
        // given: three members; A waits on C; A and C vote to exclude B
        let mut w = setup(3);
        w.forget(A, C, 100);
        // when: tallied
        let ledger = w.exclude(A, B, 20).exclude(C, B, 30).ledger();
        // then: B is excluded, two are left and A alone forgets C
        assert_eq!(
            shown(&ledger, &lettered(&w, 3)),
            concat!(
                "Ledger { admitted: {B, A, C}, ",
                "excluded: {B: Exclusion { how: Votes, at_micros: 30000000, voters: [A, C] }}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {C: ForgetRound { voters: [A], at_unix: Some(100) }}, ",
                "admit_unix: {B: 0, C: 0}, ",
                "forget_marks: {C: 100}, ",
                "forget_need: 1 }",
            )
        );
    }
}

// --- helpers ---

/// The names `A`.. of the first `n` nodes of the scene.
fn lettered(w: &World, n: usize) -> Vec<(&'static str, PeerId)> {
    ["A", "B", "C", "D", "E", "F"]
        .into_iter()
        .zip((0..n).map(|i| w.id(i)))
        .collect()
}

/// The names `A`.. of the nodes of `net(n)`.
fn letters(peers: &[PeerId]) -> Vec<(&'static str, PeerId)> {
    ["A", "B", "C", "D", "E", "F"]
        .into_iter()
        .zip(peers.iter().copied())
        .collect()
}
