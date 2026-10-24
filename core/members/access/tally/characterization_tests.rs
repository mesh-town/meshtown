//! Snapshots of the tally result (`Ledger`) for admission and exclusion scenarios: the input is
//! built record by record and the whole `Debug` text of the ledger is compared, with node ids replaced by
//! the names of the scene. Cascades: `characterization_cascade_tests.rs`, forget scenarios:
//! `characterization_forget_tests.rs`.

use super::tests::{cast, members, scene, Cast};
use super::*;
use crate::transport::net::PeerId;

mod admission {
    use super::*;

    #[test]
    fn me_admits_a_newcomer() {
        // given: M admits X
        let c = cast();
        let s = scene(c.m).admit_via(c.m, c.x, Via::Invite);
        // when: tallied
        let ledger = s.ledger();
        // then: X is listed at once, nothing waits
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {m, x}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {x: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 1, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn stranger_admitting_itself_and_others_is_ignored() {
        // given: M admits B; X, whom nobody admitted, admits itself and A
        let c = cast();
        let s = members(c.m, &[c.b]).admit(c.x, c.x).admit(c.x, c.a);
        // when: tallied
        let ledger = s.ledger();
        // then: only M and B are listed, there is no request
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {m, b}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 1, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, b: 0, x: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 1, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn admit_by_unlisted_author_is_ignored() {
        // given: M admits A; X, whom nobody admitted, admits B
        let c = cast();
        let s = members(c.m, &[c.a]).admit(c.x, c.b);
        // when: tallied
        let ledger = s.ledger();
        // then: B is neither listed nor a request
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m}, ",
                "excluded: {}, ",
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

    #[test]
    fn my_witness_admits_at_me() {
        // given: M writes witnesses for A and C, A admits B
        let c = cast();
        let s = scene(c.m)
            .admit_via(c.m, c.a, Via::Witness)
            .admit_via(c.m, c.c, Via::Witness)
            .admit(c.a, c.b);
        // when: tallied at M
        let ledger = s.ledger();
        // then: A and C are listed through the witness, B through the vote of A
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, c, b}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn witness_of_others_is_a_request_not_a_vote() {
        // given: three members; A and B both write witnesses for X
        let c = cast();
        let s = members(c.m, &[c.a, c.b])
            .admit_via(c.a, c.x, Via::Witness)
            .admit_via(c.b, c.x, Via::Witness);
        // when: tallied at M
        let ledger = s.ledger();
        // then: X is not listed, it waits as a request without votes, and the exclusion need stays two
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, b}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {x: Admission { voters: [], need: 1, label: None }}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn one_vote_admits_in_a_network_of_three() {
        // given: three members that admit each other; only A invites X
        let c = cast();
        let s = members(c.m, &[c.a, c.b])
            .admit(c.a, c.m)
            .admit(c.a, c.b)
            .admit(c.b, c.m)
            .admit(c.b, c.a)
            .admit_via(c.a, c.x, Via::Invite);
        // when: tallied
        let ledger = s.ledger();
        // then: the single vote admits X while exclusion needs two
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, b, x}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, m: 0, b: 0, x: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn newcomer_sees_its_own_request() {
        // given: I am X; I wrote witnesses for A and B, who know each other; A writes a witness for me
        let c = cast();
        let s = scene(c.x)
            .admit_via(c.x, c.a, Via::Witness)
            .admit_via(c.x, c.b, Via::Witness)
            .admit(c.a, c.b)
            .admit(c.b, c.a)
            .admit_via(c.a, c.x, Via::Witness);
        // when: tallied at X
        let ledger = s.ledger();
        // then: X is listed through its own witnesses and its request has no votes
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, b, x}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {x: Admission { voters: [], need: 1, label: None }}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }
}

mod exclusion {
    use super::*;

    #[test]
    fn two_votes_exclude_one_and_a_third_waits_in_five() {
        // given: five members; A and B vote against D, A votes against C
        let c = cast();
        let s = members(c.m, &[c.a, c.b, c.c, c.d])
            .vote(c.a, c.d, 10)
            .vote(c.b, c.d, 20)
            .vote(c.a, c.c, 30);
        // when: tallied
        let ledger = s.ledger();
        // then: D is excluded by both in time order, C has the single vote of A pending
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {d, a, m, c, b}, ",
                "excluded: {d: Exclusion { how: Votes, at_micros: 20, voters: [a, b] }}, ",
                "pending: {c: PendingVote { voters: [a], need: 2 }}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {d: 0, a: 0, c: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn leaving_excludes_with_the_vote_for_oneself() {
        // given: five members; A votes against itself
        let c = cast();
        let s = members(c.m, &[c.a, c.b, c.c, c.d]).vote(c.a, c.a, 5);
        // when: tallied
        let ledger = s.ledger();
        // then: A is excluded as having left, the others stay
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {d, a, m, c, b}, ",
                "excluded: {a: Exclusion { how: Left, at_micros: 5, voters: [a] }}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {d: 0, a: 0, c: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn colluding_pair_backdates_against_proposer_in_five() {
        // given: B admits A, C, M, X; M and X exclude A at 20; A votes against M at 100; B at 200
        let c = cast();
        let s = members(c.b, &[c.a, c.c, c.m, c.x])
            .vote(c.m, c.a, 10)
            .vote(c.x, c.a, 20)
            .vote(c.a, c.m, 100)
            .vote(c.b, c.m, 200);
        // when: tallied at B
        let ledger = s.ledger();
        // then: A is out at 20, its vote against M is void, M waits with the single vote of B
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, c, b, x}, ",
                "excluded: {a: Exclusion { how: Votes, at_micros: 20, voters: [m, x] }}, ",
                "pending: {m: PendingVote { voters: [b], need: 2 }}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, m: 0, c: 0, x: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }

    #[test]
    fn votes_of_strangers_and_on_strangers_are_ignored() {
        // given: three members; X votes against A, and A votes against X
        let c = cast();
        let s = members(c.m, &[c.a, c.b])
            .vote(c.x, c.a, 10)
            .vote(c.a, c.x, 20);
        // when: tallied
        let ledger = s.ledger();
        // then: nobody is excluded and nothing is pending
        assert_eq!(
            shown(&ledger, &names(&c)),
            concat!(
                "Ledger { admitted: {a, m, b}, ",
                "excluded: {}, ",
                "pending: {}, ",
                "need: 2, ",
                "admissions: {}, ",
                "forgets: {}, ",
                "admit_unix: {a: 0, b: 0}, ",
                "forget_marks: {}, ",
                "forget_need: 2, ",
                "admins: {}, closed: None }",
            )
        );
    }
}

// --- helpers ---

/// The names of the standard cast.
pub(super) fn names(c: &Cast) -> Vec<(&'static str, PeerId)> {
    vec![
        ("m", c.m),
        ("a", c.a),
        ("b", c.b),
        ("c", c.c),
        ("d", c.d),
        ("x", c.x),
    ]
}

/// The `Debug` text of the ledger with every node id replaced by its name.
pub(super) fn shown(ledger: &Ledger, names: &[(&str, PeerId)]) -> String {
    let mut text = format!("{ledger:?}");
    for (name, id) in names {
        text = text.replace(&format!("{id:?}"), name);
    }
    text
}
