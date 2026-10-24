//! More tests of one pass over the records of an administered network (`roles.rs`): exclusion,
//! return, leaving and closing. The scene helpers are shared with `roles_tests.rs`.

use super::tests::{replay, setup, A, B, C, ROOT, X, Z};
use crate::members::access::tally::How;
use crate::transport::net::contract::pid;

mod excluding {
    use super::*;

    #[test]
    fn admin_excludes_with_one_vote() {
        // given: A is an admin and votes to exclude the member X
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(X))
            .exclude(40, pid(A), pid(X));
        // when
        let r = replay(&scene);
        // then: X is out by that single vote, which is named
        assert!(!r.members.contains(&pid(X)));
        let exclusion = &r.excluded[&pid(X)];
        assert_eq!(exclusion.how, How::Votes);
        assert_eq!(exclusion.voters, vec![pid(A)]);
        assert_eq!(exclusion.at_micros, 40_000_000);
    }

    #[test]
    fn exclude_root_is_ignored() {
        // given: an admin votes to exclude the root
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .exclude(30, pid(A), pid(ROOT));
        // when / then: the root is a member and an admin still
        let r = replay(&scene);
        assert!(r.members.contains(&pid(ROOT)) && r.admins.contains(&pid(ROOT)));
        assert!(r.excluded.is_empty());
    }

    #[test]
    fn non_admins_cannot_exclude_admin() {
        // given: B and C, plain members, both vote to exclude the admin A
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(B))
            .admit(10, pid(ROOT), pid(C))
            .exclude(40, pid(B), pid(A))
            .exclude(41, pid(C), pid(A));
        // when / then: A is a member and an admin, nobody is excluded
        let r = replay(&scene);
        assert!(r.members.contains(&pid(A)) && r.admins.contains(&pid(A)));
        assert!(r.excluded.is_empty());
    }
}

mod returning {
    use super::*;

    #[test]
    fn later_admit_returns_excluded() {
        // given: the root admits X, excludes it, then admits it again
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(X))
            .exclude(20, pid(ROOT), pid(X))
            .admit(30, pid(ROOT), pid(X));
        // when / then: X is a member and no longer excluded
        let r = replay(&scene);
        assert!(r.members.contains(&pid(X)));
        assert!(!r.excluded.contains_key(&pid(X)));
    }

    #[test]
    fn earlier_admit_does_not_return() {
        // given: the root and the admin A admit X before the root excludes it
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(10, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(X))
            .admit(20, pid(A), pid(X))
            .exclude(30, pid(ROOT), pid(X));
        // when / then: X is excluded, the older admissions do not bring it back
        let r = replay(&scene);
        assert!(!r.members.contains(&pid(X)));
        assert!(r.excluded.contains_key(&pid(X)));
    }

    #[test]
    fn returned_admin_is_plain_member() {
        // given: A was an admin, was excluded, and was admitted again
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .exclude(30, pid(ROOT), pid(A))
            .admit(40, pid(ROOT), pid(A));
        // when / then: A is a member, the old grant does not come back
        let r = replay(&scene);
        assert!(r.members.contains(&pid(A)));
        assert!(!r.admins.contains(&pid(A)));
    }

    #[test]
    fn same_ts_admit_and_exclude_excludes() {
        // given: X is out; at the same instant the admin A admits it and the root excludes it
        let mut scene = setup();
        scene
            .admit(5, pid(ROOT), pid(A))
            .grant(5, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(X))
            .exclude(15, pid(ROOT), pid(X))
            .admit(20, pid(A), pid(X))
            .exclude(20, pid(ROOT), pid(X));
        // when / then: the removal wins the tie
        let r = replay(&scene);
        assert!(!r.members.contains(&pid(X)));
        assert!(r.excluded.contains_key(&pid(X)));
    }
}

mod leaving {
    use super::*;

    #[test]
    fn left_is_final() {
        // given: A leaves, then the root admits A again
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .leave(20, pid(A))
            .admit(30, pid(ROOT), pid(A));
        // when
        let r = replay(&scene);
        // then: A stays out, and it is told apart from an exclusion by votes
        assert!(!r.members.contains(&pid(A)));
        assert_eq!(r.excluded[&pid(A)].how, How::Left);
        assert!(r.left.contains(&pid(A)));
    }

    #[test]
    fn root_left_keeps_its_members() {
        // given: the root admits A and B and leaves
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .admit(11, pid(ROOT), pid(B))
            .leave(20, pid(ROOT));
        // when
        let r = replay(&scene);
        // then: the admitted stay, the root is gone and no longer an admin
        assert!(r.members.contains(&pid(A)) && r.members.contains(&pid(B)));
        assert!(!r.members.contains(&pid(ROOT)) && !r.admins.contains(&pid(ROOT)));
    }
}

mod closing {
    use super::*;

    #[test]
    fn close_by_admin() {
        // given: the admin A closes the network at 50 s and 999 us
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .close_micros(50_000_999, pid(A));
        // when
        let closed = replay(&scene).closed.expect("a close by an admin counts");
        // then: the author and the whole seconds of the record
        assert_eq!((closed.by, closed.at_unix), (pid(A), 50));
    }

    #[test]
    fn close_by_non_admin_is_none() {
        // given: a plain member closes the network
        let mut scene = setup();
        scene.admit(10, pid(ROOT), pid(A)).close(20, pid(A));
        // when / then: the network is open
        assert!(replay(&scene).closed.is_none());
    }

    #[test]
    fn records_after_close_do_not_count() {
        // given: the root admits Z before and X after closing the network
        let mut scene = setup();
        scene
            .admit(20, pid(ROOT), pid(Z))
            .close(30, pid(ROOT))
            .admit(40, pid(ROOT), pid(X));
        // when
        let r = replay(&scene);
        // then: the earlier admission stands, the later one does not
        assert!(r.members.contains(&pid(Z)) && !r.members.contains(&pid(X)));
        assert!(r.closed.is_some());
    }

    #[test]
    fn first_close_wins() {
        // given: the root closes at 30, the admin A at 40
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .close(40, pid(A))
            .close(30, pid(ROOT));
        // when
        let closed = replay(&scene).closed.expect("closed");
        // then: the earlier record names the closer, whatever the order of the input
        assert_eq!((closed.by, closed.at_unix), (pid(ROOT), 30));
    }
}
