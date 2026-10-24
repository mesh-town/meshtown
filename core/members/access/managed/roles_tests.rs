//! Tests of one pass over the records of an administered network (`roles.rs`): who is a member, who is
//! an administrator, who is excluded, whether the network is closed. No pinned exclusions here; the
//! fixed point over voided records is in `closure_tests.rs`.

use std::collections::BTreeSet;

use super::super::events::events;
use super::*;
// assumed: the support module of the managed tests is `managed::support`.
use crate::members::access::managed::support::Scene;
use crate::members::access::tally::How;
use crate::transport::net::contract::pid;

// Nodes of the scenes: the root and four more, plus a newcomer `x`.
const ROOT: u8 = 1;
const A: u8 = 2;
const B: u8 = 3;
const C: u8 = 4;
const X: u8 = 5;
const Z: u8 = 6;

mod admission {
    use super::*;

    #[test]
    fn root_admits_member() {
        // given: the root admits X
        let mut scene = setup();
        scene.admit(10, pid(ROOT), pid(X));
        // when: the records are replayed
        let r = replay(&scene);
        // then: X is a member and not an administrator; the root is both from the start
        assert!(r.members.contains(&pid(X)) && r.admitted.contains(&pid(X)));
        assert!(!r.admins.contains(&pid(X)));
        assert_eq!(r.admins, BTreeSet::from([pid(ROOT)]));
    }

    #[test]
    fn non_admin_admit_is_ignored() {
        // given: A is a plain member and admits Z
        let mut scene = setup();
        scene.admit(10, pid(ROOT), pid(A)).admit(20, pid(A), pid(Z));
        // when / then: Z is not a member
        let r = replay(&scene);
        assert!(r.members.contains(&pid(A)));
        assert!(!r.members.contains(&pid(Z)), "an admission by a non-admin");
    }

    #[test]
    fn witness_never_admits() {
        // given: A is an admin, B a plain member; A, B and the root vouch for Z, A vouches for itself
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(B))
            .witness(30, pid(A), pid(Z))
            .witness(31, pid(B), pid(C))
            .witness(32, pid(ROOT), pid(X))
            .witness(33, pid(A), pid(A));
        // when
        let r = replay(&scene);
        // then: nobody became a member through a witness record
        let strangers = [pid(Z), pid(C), pid(X)];
        assert!(strangers.iter().all(|p| !r.members.contains(p)));
        assert_eq!(r.members.len(), 3, "the root, A and B only");
    }

    #[test]
    fn witness_of_admin_is_a_request() {
        // given: A is an admin, B a plain member; each vouches for a stranger
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(B))
            .witness(30, pid(A), pid(Z))
            .witness(31, pid(B), pid(C));
        // when: the ledger is taken
        let ledger = scene.ledger();
        // then: only the vouching of the admin is a request for the admins to answer
        assert!(ledger.admissions.contains_key(&pid(Z)));
        assert!(!ledger.admissions.contains_key(&pid(C)));
    }
}

mod granting {
    use super::*;

    #[test]
    fn grant_chain_root_a_b() {
        // given: the root makes A an admin, A admits B and makes B an admin
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .admit(30, pid(A), pid(B))
            .grant(40, pid(A), pid(B));
        // when / then: the admins are the root, A and B
        let r = replay(&scene);
        let want = BTreeSet::from([pid(ROOT), pid(A), pid(B)]);
        assert_eq!(r.admins, want);
        assert!(r.members.contains(&pid(B)));
    }

    #[test]
    fn grant_before_admission_is_ignored() {
        // given: the root grants A before A is a member, then admits A
        let mut scene = setup();
        scene
            .grant(10, pid(ROOT), pid(A))
            .admit(20, pid(ROOT), pid(A));
        // when / then: A is a member but the early grant gave nothing
        let r = replay(&scene);
        assert!(r.members.contains(&pid(A)));
        assert!(!r.admins.contains(&pid(A)));
    }

    #[test]
    fn self_grant_by_member_is_ignored() {
        // given: a plain member A grants itself
        let mut scene = setup();
        scene.admit(10, pid(ROOT), pid(A)).grant(20, pid(A), pid(A));
        // when / then: A stays a plain member
        assert!(!replay(&scene).admins.contains(&pid(A)));
    }

    #[test]
    fn backdated_grant_is_ignored() {
        // given: A becomes an admin at 30 and writes a grant of B dated 20
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(B))
            .grant(30, pid(ROOT), pid(A))
            .grant(20, pid(A), pid(B));
        // when / then: at 20 A was not an admin, so B is not one
        assert!(!replay(&scene).admins.contains(&pid(B)));
    }
}

mod revoking {
    use super::*;

    #[test]
    fn revoke_cuts_forward_only() {
        // given: A is an admin from 20 to 40; A admits B at 30 and C at 50
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .admit(30, pid(A), pid(B))
            .revoke(40, pid(ROOT), pid(A))
            .admit(50, pid(A), pid(C));
        // when
        let r = replay(&scene);
        // then: the admission made in time stands, the later one does not, A is no admin
        assert!(r.members.contains(&pid(B)));
        assert!(!r.members.contains(&pid(C)));
        assert!(!r.admins.contains(&pid(A)) && r.members.contains(&pid(A)));
    }

    #[test]
    fn revoke_root_is_ignored() {
        // given: A, an admin, revokes the root; the root revokes itself
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .revoke(30, pid(A), pid(ROOT))
            .revoke(40, pid(ROOT), pid(ROOT));
        // when / then: the root is an admin still
        assert!(replay(&scene).admins.contains(&pid(ROOT)));
    }

    #[test]
    fn revoke_last_admin_after_root_left_is_ignored() {
        // given: A is the only admin after the root has left; A revokes itself
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .leave(30, pid(ROOT))
            .revoke(40, pid(A), pid(A));
        // when / then: the network keeps an admin
        assert_eq!(replay(&scene).admins, BTreeSet::from([pid(A)]));
    }

    #[test]
    fn self_revoke_with_another_admin() {
        // given: the root and A are admins; A revokes itself
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .revoke(30, pid(A), pid(A));
        // when / then: A is a plain member, the root an admin
        let r = replay(&scene);
        assert_eq!(r.admins, BTreeSet::from([pid(ROOT)]));
        assert!(r.members.contains(&pid(A)));
    }
}

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

// --- setup and helpers ---

/// A network created by the node `ROOT`.
fn setup() -> Scene {
    Scene::new(pid(ROOT))
}

/// One pass over the records of the scene, nothing pinned.
fn replay(scene: &Scene) -> Replay {
    Replay::run(&events(&scene.input()), scene.root, &BTreeSet::new())
}
