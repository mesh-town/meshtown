//! Tests of one pass over the records of an administered network (`roles.rs`): who is a member, who is
//! an administrator, who is excluded, whether the network is closed. No pinned exclusions here; the
//! fixed point over voided records is in `closure_tests.rs`.

use std::collections::BTreeSet;

use super::super::events::events;
use super::*;
// assumed: the support module of the managed tests is `managed::support`.
use crate::members::access::managed::support::Scene;
use crate::transport::net::contract::pid;

// Nodes of the scenes: the root and four more, plus a newcomer `x`.
pub(super) const ROOT: u8 = 1;
pub(super) const A: u8 = 2;
pub(super) const B: u8 = 3;
pub(super) const C: u8 = 4;
pub(super) const X: u8 = 5;
pub(super) const Z: u8 = 6;

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

// --- setup and helpers ---

/// A network created by the node `ROOT`.
pub(super) fn setup() -> Scene {
    Scene::new(pid(ROOT))
}

/// One pass over the records of the scene, nothing pinned.
pub(super) fn replay(scene: &Scene) -> Replay {
    Replay::run(&events(&scene.input()), scene.root, &BTreeSet::new())
}
