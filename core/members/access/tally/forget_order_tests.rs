//! Tests of the forget pass inside the tally (`pass.rs`, `forget.rs`), part two: what the forgotten
//! author did before, an excluded node, and order independence. Who is forgotten and who returns:
//! `forget_tests.rs`.

use super::support::*;
use super::*;
use crate::transport::net::contract::pid;

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
