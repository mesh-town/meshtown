//! Snapshots of the tally result (`Ledger`) for scenarios where an exclusion takes the admissions and the
//! waiting votes of the excluded node with it: the whole `Debug` text of the ledger is compared, node ids replaced by names.

use super::characterization_tests::{names, shown};
use super::tests::{cast, members};
use super::*;

mod cascade {
    use super::*;

    #[test]
    fn admits_of_an_excluded_author_stop_counting() {
        // given: M admits A and B, A admits D; M and B vote against A
        let c = cast();
        let s = members(c.m, &[c.a, c.b])
            .admit(c.a, c.d)
            .vote(c.m, c.a, 10)
            .vote(c.b, c.a, 20);
        // when: tallied
        let ledger = s.ledger();
        // then: A is out, D, admitted by A alone, is neither listed nor a request
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, b}, ",
                "excluded: {a: Exclusion { how: Votes, at_micros: 20, voters: [m, b] }}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {d: 0, a: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 1, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn second_admitter_keeps_the_subject() {
        // given: the same, but B admitted D as well
        let c = cast();
        let s = members(c.m, &[c.a, c.b]).admit(c.a, c.d).admit(c.b, c.d);
        let s = s.vote(c.m, c.a, 10).vote(c.b, c.a, 20);
        // when: tallied
        let ledger = s.ledger();
        // then: D stays listed
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {d, a, m, b}, ",
                "excluded: {a: Exclusion { how: Votes, at_micros: 20, voters: [m, b] }}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {d: 0, a: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn request_of_an_excluded_author_is_gone() {
        // given: four members; A writes a witness for X
        let c = cast();
        let base = members(c.m, &[c.a, c.b, c.c]).admit_via(c.a, c.x, Via::Witness);
        // when: tallied before and after M and C exclude A
        let before = base.ledger();
        let after = base.vote(c.m, c.a, 10).vote(c.c, c.a, 20).ledger();
        // then: X waits while A is open, and shows up in no request once A is excluded
        assert_eq!(
            shown(&before, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, c, b}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {x: Admission { voters: [], need: 1, label: None }}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, c: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
        assert_eq!(
            shown(&after, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, c, b}, ",
                "excluded: {a: Exclusion { how: Votes, at_micros: 20, voters: [m, c] }}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, c: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn excluded_subject_is_not_a_request() {
        // given: four members; A invites X, B writes a witness for X; X leaves, then A falls
        let c = cast();
        let s = members(c.m, &[c.a, c.b, c.c])
            .admit_via(c.a, c.x, Via::Invite)
            .admit_via(c.b, c.x, Via::Witness)
            .vote(c.x, c.x, 5)
            .vote(c.m, c.a, 10)
            .vote(c.c, c.a, 20);
        // when: tallied
        let ledger = s.ledger();
        // then: X stays recorded as excluded though B still backs it, and it is in no request
        assert_eq!(shown(&ledger, &names(&c)), concat!(
                "Ledger { admitted: {a, m, c, b}, ",
                "excluded: {a: Exclusion { how: Votes, at_micros: 20, voters: [m, c] }, x: Exclusion { how: Left, at_micros: 5, voters: [x] }}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, c: 0, b: 0, x: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            ));
    }

    #[test]
    fn my_witness_keeps_a_member_after_the_cascade() {
        // given: A admits X, M writes a witness for X; M and C exclude A
        let c = cast();
        let s = members(c.m, &[c.a, c.b, c.c])
            .admit_via(c.a, c.x, Via::Invite)
            .admit_via(c.m, c.x, Via::Witness)
            .vote(c.m, c.a, 10)
            .vote(c.c, c.a, 20);
        // when: tallied
        let ledger = s.ledger();
        // then: X stays listed through the witness of M
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, c, b, x}, ",
                "excluded: {a: Exclusion { how: Votes, at_micros: 20, voters: [m, c] }}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, c: 0, b: 0, x: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn admits_of_the_own_node_count_after_it_left() {
        // given: M admits A and B, then M leaves
        let c = cast();
        let s = members(c.m, &[c.a, c.b]).vote(c.m, c.m, 10);
        // when: tallied
        let ledger = s.ledger();
        // then: M is excluded as having left, A and B stay listed
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, b}, ",
                "excluded: {m: Exclusion { how: Left, at_micros: 10, voters: [m] }}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 1, ",
                "admins: {}, closed: None }",
            )
        );
    }
}

mod refresh {
    use super::*;

    #[test]
    fn exclusion_beats_admit_written_later() {
        // given: A is admitted by M, B and C; B and C vote against A
        let c = cast();
        let s = members(c.m, &[c.a, c.b, c.c])
            .admit(c.b, c.a)
            .admit(c.c, c.a);
        let s = s.vote(c.b, c.a, 10).vote(c.c, c.a, 20);
        // when: tallied
        let ledger = s.ledger();
        // then: A is excluded however many admit it
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, c, b}, ",
                "excluded: {a: Exclusion { how: Votes, at_micros: 20, voters: [b, c] }}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, c: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn pending_votes_of_an_excluded_voter_drop() {
        // given: four members; A votes against C at 10, then M and B exclude A
        let c = cast();
        let s = members(c.m, &[c.a, c.b, c.c])
            .vote(c.a, c.c, 10)
            .vote(c.m, c.a, 20)
            .vote(c.b, c.a, 30);
        // when: tallied
        let ledger = s.ledger();
        // then: A is out and its waiting vote against C is gone
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, c, b}, ",
                "excluded: {a: Exclusion { how: Votes, at_micros: 30, voters: [m, b] }}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, c: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn need_drops_to_one_when_the_network_shrinks_to_two() {
        // given: M, A, B; A votes against B at 10 (waits), M leaves at 20
        let c = cast();
        let s = members(c.m, &[c.a, c.b])
            .vote(c.a, c.b, 10)
            .vote(c.m, c.m, 20);
        // when: tallied
        let ledger = s.ledger();
        // then: the waiting vote excludes B at the moment M left, one vote is enough
        assert_eq!(shown(&ledger, &names(&c)), concat!(
                "Ledger { admitted: {a, m, b}, ",
                "excluded: {m: Exclusion { how: Left, at_micros: 20, voters: [m] }, b: Exclusion { how: Votes, at_micros: 20, voters: [a] }}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 1, ",
                "admins: {}, closed: None }",
            ));
    }
}
