//! Tests of the exclusion threshold taken at the moment of the vote: a vote with the mark is counted against
//! the nodes that were active then, so a node admitted later does not lapse the exclusion. A vote without the
//! mark keeps the current threshold.

use super::support::*;
use super::*;
use crate::transport::net::contract::pid;

#[path = "sticky_cases_tests.rs"]
mod cases;

mod network_of_two {
    use super::*;

    #[test]
    fn two_nodes_exclusion_survives_a_newcomer() {
        // given: A and B, A excludes B at 10 s with the mark, then C is admitted at 50 s
        let mut world = setup(3);
        world.admitted_at(C, 50).exclude_at_moment(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: B is excluded by the vote of A alone, C is a member, two nodes are active now
        assert_excluded_at(&world, &ledger, B, 10);
        assert_eq!(ledger.excluded[&world.id(B)].voters, vec![world.id(A)]);
        assert!(ledger.is_member(&world.id(C)));
        assert_eq!(ledger.need, 1, "A and C are active");
    }

    #[test]
    fn two_nodes_exclusion_survives_growth_to_five() {
        // given: A and B, A excludes B at 10 s with the mark, then C, D and E are admitted
        let mut world = setup(5);
        world
            .admitted_at(C, 50)
            .admitted_at(D, 60)
            .admitted_at(E, 70);
        world.exclude_at_moment(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: B stays excluded, four nodes are active, two votes are needed from now on
        assert_excluded_at(&world, &ledger, B, 10);
        assert_eq!(ledger.need, 2);
    }

    #[test]
    fn newcomer_view_keeps_the_exclusion() {
        // given: the view of C: A admitted B at 0 and C at 50, C confirmed A at 51; A excluded B at 10
        let (a, b, c) = (pid(1), pid(2), pid(3));
        let admits = vec![
            admit_at(a, b, Via::Invite, 0),
            admit_at(a, c, Via::Invite, 50),
            admit_at(c, a, Via::Witness, 51),
        ];
        let votes = vec![vote_at_moment(a, b, 10)];
        // when: C tallies
        let ledger = run_votes(c, admits, vec![], votes);
        // then: the founder A was always there, so B is excluded for C as well
        assert!(ledger.excluded.contains_key(&b), "B is excluded for C");
        assert!(ledger.is_member(&a) && ledger.is_member(&c));
    }
}

mod vote_without_the_mark {
    use super::*;

    #[test]
    fn old_vote_in_two_lapses_when_a_newcomer_joins() {
        // given: A and B, A excludes B at 10 s without the mark, then C is admitted at 50 s
        let mut world = setup(3);
        world.admitted_at(C, 50).exclude(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: the current rule holds: three active, two votes needed, B is a member again
        assert!(ledger.is_member(&world.id(B)));
        assert_eq!(ledger.pending[&world.id(B)].voters, vec![world.id(A)]);
        assert_eq!(ledger.need, 2);
    }

    #[test]
    fn old_vote_in_two_still_excludes_without_newcomer() {
        // given: A and B, A excludes B at 10 s without the mark
        let mut world = setup(2);
        world.exclude(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: one vote of two is enough
        assert_excluded_at(&world, &ledger, B, 10);
    }
}

mod threshold_at_the_moment {
    use super::*;

    #[test]
    fn single_vote_waits_in_three() {
        // given: three nodes admitted at 0; A excludes B at 10 s with the mark
        let mut world = setup(3);
        world.exclude_at_moment(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: the vote waits for the second one
        assert_waits(&world, &ledger, B);
        assert_eq!(ledger.need, 2);
    }

    #[test]
    fn two_of_three_stay_excluded_as_network_grows() {
        // given: five nodes; D and E are admitted at 30 and 40 s; A and C vote against B at 10 and 20 s
        let world = scene_of_two_votes();
        // when: tallied
        let ledger = world.ledger();
        // then: three were active at the second vote, two votes were enough
        assert_excluded_at(&world, &ledger, B, 20);
        assert_eq!(
            ledger.excluded[&world.id(B)].voters,
            vec![world.id(A), world.id(C)]
        );
    }

    #[test]
    fn no_votes_keep_everyone_in_a_grown_network() {
        // given: three nodes, C admitted at 50 s, nobody voted
        let mut world = setup(3);
        world.admitted_at(C, 50);
        // when: tallied
        let ledger = world.ledger();
        // then: everyone is a member
        let all = [A, B, C].map(|who| world.id(who));
        assert!(all.iter().all(|p| ledger.is_member(p)));
        assert!(ledger.excluded.is_empty());
    }

    #[test]
    fn admission_in_the_vote_second_counts() {
        // given: C is admitted at 10 s, A excludes B at the same second with the mark
        let mut world = setup(3);
        world.admitted_at(C, 10).exclude_at_moment(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: C is counted, three are active, the vote waits
        assert_waits(&world, &ledger, B);
    }

    #[test]
    fn admission_stamped_before_the_vote_counts() {
        // given: C is admitted at 5 s, A excludes B at 10 s with the mark
        let mut world = setup(3);
        world.admitted_at(C, 5).exclude_at_moment(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: C is counted, the vote waits
        assert_waits(&world, &ledger, B);
    }

    #[test]
    fn vote_before_subject_admission_uses_its_admission() {
        // given: B and C are admitted at 20 s, but the vote of A is stamped 10 s (clocks differ)
        let mut world = setup(3);
        world.admitted_at(B, 20).admitted_at(C, 20);
        world.exclude_at_moment(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: the moment is lifted to the admission of B, three are active, the vote waits
        assert_waits(&world, &ledger, B);
    }

    #[test]
    fn vote_before_voter_admission_uses_its_admission() {
        // given: C is admitted at 20 s, but the vote of C is stamped 10 s (clocks differ)
        let mut world = setup(3);
        world.admitted_at(C, 20).exclude_at_moment(C, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: the moment is lifted to the admission of C, three are active, the vote waits
        assert_waits(&world, &ledger, B);
    }

    #[test]
    fn earliest_admission_dates_a_node() {
        // given: C is admitted by A at 50 s and by B at 5 s; A excludes B at 10 s with the mark
        let mut world = setup(3);
        let c = world.id(C);
        world.admitted_at(C, 50).admit(B, c, Via::Invite, 5);
        world.exclude_at_moment(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: C counts from 5 s, three are active, the vote waits
        assert_waits(&world, &ledger, B);
    }

    #[test]
    fn witness_admission_does_not_date_a_node() {
        // given: C is admitted by A at 50 s and only witnessed by B at 5 s; A excludes B at 10 s
        let mut world = setup(3);
        let c = world.id(C);
        world.admitted_at(C, 50).admit(B, c, Via::Witness, 5);
        world.exclude_at_moment(A, B, 10);
        // when: tallied
        let ledger = world.ledger();
        // then: C counts from 50 s, two are active, one vote is enough
        assert_excluded_at(&world, &ledger, B, 10);
    }
}

// --- helpers ---

/// Five nodes; D and E are admitted at 30 and 40 s; A and C vote against B at 10 and 20 s with the mark.
fn scene_of_two_votes() -> World {
    let mut world = setup(5);
    world.admitted_at(D, 30).admitted_at(E, 40);
    world
        .exclude_at_moment(A, B, 10)
        .exclude_at_moment(C, B, 20);
    world
}

fn assert_excluded_at(world: &World, ledger: &Ledger, who: usize, secs: u64) {
    let excluded = ledger.excluded.get(&world.id(who));
    let at = excluded.map(|e| (e.how, e.at_micros));
    assert_eq!(
        at,
        Some((How::Votes, secs * 1_000_000)),
        "excluded by votes"
    );
}

fn assert_waits(world: &World, ledger: &Ledger, who: usize) {
    let p = world.id(who);
    assert!(ledger.is_member(&p), "the node is still a member");
    assert!(
        ledger.pending.contains_key(&p),
        "the vote waits for another"
    );
}
