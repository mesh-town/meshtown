//! Snapshots of the tally result (`Ledger`) for the return of a forgotten node (see `characterization_forget_tests.rs`).

use super::characterization_forget_tests::letters;
use super::characterization_tests::shown;
use super::support::*;
use super::*;

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
                "forget_need: 1, ",
                "admins: {}, closed: None }",
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
                "forget_need: 1, ",
                "admins: {}, closed: None }",
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
                "forget_need: 1, ",
                "admins: {}, closed: None }",
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
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            ));
    }
}
