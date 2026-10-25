//! Tests of the moment threshold, part two: the shrink of the network, the leave, the order of the input
//! and the list that mixes marked and unmarked votes. The thresholds themselves: `sticky_tests.rs`.

use super::*;

mod shrink_and_growth {
    use super::*;

    #[test]
    fn shrink_then_growth_keeps_exclusion() {
        // given: four nodes, D is admitted at 30 s; A votes against B at 10 s, C leaves at 20 s
        let mut world = setup(4);
        world.admitted_at(D, 30).exclude_at_moment(A, B, 10);
        world.exclude_at_moment(C, C, 20);
        // when: tallied
        let ledger = world.ledger();
        // then: at 20 s only A and B were active, so the vote of A excludes B; D is a member
        assert_excluded_at(&world, &ledger, B, 20);
        assert_eq!(ledger.excluded[&world.id(C)].how, How::Left);
        assert!(ledger.is_member(&world.id(D)));
    }
}

mod leave {
    use super::*;

    #[test]
    fn leave_stays_left_after_growth() {
        for marked in [false, true] {
            // given: three nodes, C is admitted at 50 s; B leaves at 10 s, with or without the mark
            let mut world = setup(3);
            world.admitted_at(C, 50);
            match marked {
                true => world.exclude_at_moment(B, B, 10),
                false => world.exclude(B, B, 10),
            };
            // when: tallied
            let ledger = world.ledger();
            // then: B has left and stays so
            let left = &ledger.excluded[&world.id(B)];
            assert_eq!(left.how, How::Left, "marked: {marked}");
            assert!(!ledger.is_member(&world.id(B)), "marked: {marked}");
        }
    }
}

mod determinism {
    use super::*;

    #[test]
    fn marked_votes_ledger_ignores_input_order() {
        // given: five nodes, D and E admitted at 30 and 40 s, marked votes of A and C against B
        let (a, p, admits) = net(5);
        let admits = rewrite(admits, a, p[D], Via::Invite, 30);
        let admits = rewrite(admits, a, p[E], Via::Invite, 40);
        let votes = vec![vote_at_moment(a, p[B], 10), vote_at_moment(p[C], p[B], 20)];
        let base = run_votes(a, admits.clone(), vec![], votes.clone());
        assert!(
            base.excluded.contains_key(&p[B]),
            "the input must exercise the pass"
        );
        // when: the same records come in other orders; then: the ledger is the same
        let reversed = run_votes(a, rev(&admits), vec![], rev(&votes));
        assert_eq!(reversed, base);
        assert_eq!(run_votes(a, rot(&admits, 2), vec![], rot(&votes, 1)), base);
    }
}

mod mixed_list {
    use super::*;

    #[test]
    fn mixed_list_of_two_excludes() {
        // given: C is admitted at 50 s; A votes against B at 10 s without the mark, C at 60 s with it
        let mut world = setup(3);
        world.admitted_at(C, 50).exclude(A, B, 10);
        world.exclude_at_moment(C, B, 60);
        // when: tallied
        let ledger = world.ledger();
        // then: the list is not all marked, the current threshold of two is met by two votes
        assert_excluded_at(&world, &ledger, B, 60);
        assert_eq!(
            ledger.excluded[&world.id(B)].voters,
            vec![world.id(A), world.id(C)]
        );
    }
}
