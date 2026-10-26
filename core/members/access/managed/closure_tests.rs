//! Tests of the tally of an administered network as a whole (`closure.rs`): the exclusion of an admin
//! voids what the admin wrote before, and the result does not depend on the order of the input.

use std::collections::BTreeSet;

// assumed: the support module of the managed tests is `managed::support`.
use crate::members::access::managed::support::{permutations, Scene};
use crate::members::access::tally::{tally, How, Ledger};
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

// Nodes of the scenes: the root and the others.
const ROOT: u8 = 1;
const A: u8 = 2;
const B: u8 = 3;
const C: u8 = 4;
const D: u8 = 5;
const E: u8 = 6;
const X: u8 = 7;

mod excluded_admin {
    use super::*;

    #[test]
    fn excluded_admin_voids_past_records() {
        // given: A, an admin, admits B, C and D and makes D an admin; the root admits C and D too;
        // D admits E; then the root excludes A
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .admit(30, pid(A), pid(B))
            .admit(31, pid(A), pid(C))
            .admit(32, pid(A), pid(D))
            .grant(34, pid(A), pid(D))
            .admit(33, pid(ROOT), pid(C))
            .admit(35, pid(ROOT), pid(D))
            .admit(40, pid(D), pid(E))
            .exclude(50, pid(ROOT), pid(A));
        // when
        let ledger = scene.ledger();
        // then: B had only A's word and is out; C has the root's word too and stays; D keeps its
        // membership but lost the role that A gave, so D's admission of E is void as well
        assert!(!ledger.is_member(&pid(A)));
        assert!(!ledger.is_member(&pid(B)));
        assert!(ledger.is_member(&pid(C)));
        assert!(ledger.is_member(&pid(D)) && !ledger.is_admin(&pid(D)));
        assert!(!ledger.is_member(&pid(E)));
        assert_eq!(ledger.admins, BTreeSet::from([pid(ROOT)]));
    }

    #[test]
    fn returned_node_backdated_record_is_void() {
        // given: A, an admin, is excluded at 30 and admitted again at 40; later it writes records dated
        // 25 and 30, from the time before the exclusion
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .exclude(30, pid(ROOT), pid(A))
            .admit(40, pid(ROOT), pid(A))
            .admit(25, pid(A), pid(X))
            .admit(30, pid(A), pid(B));
        // when
        let ledger = scene.ledger();
        // then: A is a plain member again and nothing dated up to the exclusion counts
        assert!(ledger.is_member(&pid(A)) && !ledger.is_admin(&pid(A)));
        assert!(!ledger.is_member(&pid(X)), "dated before the exclusion");
        assert!(!ledger.is_member(&pid(B)), "dated at the exclusion");
    }

    #[test]
    fn exclusion_by_later_excluded_admin_stands() {
        // given: A, an admin, excludes X at 40; the root excludes A at 50
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .grant(20, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(X))
            .exclude(40, pid(A), pid(X))
            .exclude(50, pid(ROOT), pid(A));
        // when
        let ledger = scene.ledger();
        // then: both are out; what A did while it counted is not undone
        assert!(!ledger.is_member(&pid(A)));
        assert!(!ledger.is_member(&pid(X)), "the exclusion of A stands");
    }

    #[test]
    fn grantee_excludes_granter() {
        // given: X makes Z an admin, and Z excludes X; the root admitted Z as well
        let (x, z) = (pid(A), pid(B));
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), x)
            .grant(20, pid(ROOT), x)
            .admit(30, x, z)
            .grant(40, x, z)
            .admit(35, pid(ROOT), z)
            .exclude(50, z, x);
        // when: the tally terminates, whatever the chain of voided records
        let ledger = scene.ledger();
        // then: X is out, Z is a plain member (the role it got from X is void)
        assert!(!ledger.is_member(&x));
        assert!(ledger.is_member(&z) && !ledger.is_admin(&z));
        assert_eq!(ledger.excluded[&x].voters, vec![z]);
    }

    #[test]
    fn close_by_admin_excluded_earlier_is_void() {
        // given: B is an admin by the grant of A; the root excludes A at 30; B closes the network at 60
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .admit(10, pid(ROOT), pid(B))
            .grant(20, pid(ROOT), pid(A))
            .grant(25, pid(A), pid(B))
            .exclude(30, pid(ROOT), pid(A))
            .close(60, pid(B));
        // when / then: B had its role from an admin who was excluded, so the close is void
        let ledger = scene.ledger();
        assert!(ledger.closed.is_none());
        assert!(ledger.is_member(&pid(B)) && !ledger.is_admin(&pid(B)));
    }
}

mod order_of_input {
    use super::*;

    #[test]
    fn input_order_does_not_matter() {
        // given: six records, among them a grantee who excludes its granter
        let steps = [
            Step::Admit(10, ROOT, A),
            Step::Grant(20, ROOT, A),
            Step::Admit(30, A, B),
            Step::Grant(40, A, B),
            Step::Exclude(50, B, A),
            Step::Admit(60, ROOT, B),
        ];
        let expected = ledger_of(&steps);
        // when: the records come in every possible order
        let orders = permutations(&steps);
        // then: the ledger is the same, and it is not the empty one
        assert_eq!(orders.len(), 720);
        for order in orders {
            assert_eq!(ledger_of(&order), expected, "{order:?}");
        }
        assert!(expected.excluded.contains_key(&pid(A)));
        assert!(expected.is_member(&pid(B)) && !expected.is_admin(&pid(B)));
        assert_eq!(expected.admins, BTreeSet::from([pid(ROOT)]));
    }
}

mod empty_input {
    use super::*;

    #[test]
    fn root_is_member_and_admin() {
        // given: no records, seen at the root
        let scene = setup();
        // when
        let ledger = scene.ledger();
        // then: the root alone is a member and an admin; nothing is waiting
        assert!(ledger.is_member(&pid(ROOT)) && ledger.is_admin(&pid(ROOT)));
        assert_eq!(ledger.admins, BTreeSet::from([pid(ROOT)]));
        assert_eq!(ledger.admitted, BTreeSet::from([pid(ROOT)]));
        assert!(ledger.excluded.is_empty() && ledger.admissions.is_empty());
    }

    #[test]
    fn other_node_is_not_a_member() {
        // given: no records, seen at a node that is not the root
        let mut scene = setup();
        scene.seen_by(pid(A));
        // when
        let ledger = scene.ledger();
        // then: the node is not a member until an admin admits it
        assert!(!ledger.is_member(&pid(A)));
        assert!(ledger.is_member(&pid(ROOT)));
    }
}

mod ledger_fields {
    use super::*;

    #[test]
    fn managed_ledger_fields() {
        // given: A is an admin and excludes X; Y is a plain member; A admits W before its role
        // began, and vouches for Z; Y vouches for V
        let (y, w, z, v) = (pid(B), pid(C), pid(D), pid(E));
        let mut scene = setup();
        scene
            .admit(10, pid(ROOT), pid(A))
            .admit(15, pid(A), w)
            .grant(20, pid(ROOT), pid(A))
            .admit(30, pid(ROOT), pid(X))
            .admit(31, pid(ROOT), y)
            .exclude(40, pid(A), pid(X))
            .witness(60, pid(A), z)
            .witness(61, y, v);
        // when
        let ledger = scene.ledger();
        // then: the roles, the exclusion by the one vote, no closing
        assert_eq!(
            ledger.admitted,
            BTreeSet::from([pid(ROOT), pid(A), pid(X), y])
        );
        assert_eq!(ledger.admins, BTreeSet::from([pid(ROOT), pid(A)]));
        let exclusion = &ledger.excluded[&pid(X)];
        assert_eq!(exclusion.how, How::Votes);
        assert_eq!(exclusion.voters, vec![pid(A)]);
        assert_eq!(ledger.excluded.len(), 1);
        assert!(ledger.closed.is_none());
        // then: a request is a record of an admin that did not count; one admin answers it
        let requests: BTreeSet<PeerId> = ledger.admissions.keys().copied().collect();
        assert_eq!(requests, BTreeSet::from([w, z]));
        assert_eq!(ledger.admissions[&z].label, None);
    }

    #[test]
    fn closed_network_reports_who_and_when() {
        // given: the root closes the network at 77 s
        let mut scene = setup();
        scene.close(77, pid(ROOT));
        // when
        let closed = scene.ledger().closed.expect("closed");
        // then
        assert_eq!((closed.by, closed.at_unix), (pid(ROOT), 77));
    }
}

mod old_forget_records {
    use super::*;
    use crate::members::access::record::tally_input;
    use crate::members::access::record::{admit_key, encode, AclKind, AclValue, Via};
    use crate::transport::net::ReplicaEntry;
    use bytes::Bytes;

    fn admit(author: u8, subject: u8, ts: u64) -> ReplicaEntry {
        let value = AclValue {
            kind: AclKind::Admit,
            subject: pid(subject),
            via: Via::Invite,
            made_unix: ts,
            note: String::new(),
        };
        ReplicaEntry {
            author: pid(author),
            key: admit_key(&pid(subject)),
            value: Bytes::from(encode(&value)),
            ts_micros: ts * 1_000_000,
        }
    }

    /// A record of the removed kind 5, assembled by hand: version 1, kind 5, subject, via 0, time, empty note.
    fn forget(author: u8, subject: u8) -> ReplicaEntry {
        let mut value = vec![1, 5];
        value.extend(pid(subject).as_bytes());
        value.extend([0, 1, 0]);
        ReplicaEntry {
            author: pid(author),
            key: forget_key_of(subject),
            value: Bytes::from(value),
            ts_micros: 50_000_000,
        }
    }

    fn forget_key_of(subject: u8) -> Vec<u8> {
        let mut key = b"acl/forget/".to_vec();
        key.extend(pid(subject).to_string().as_bytes());
        key
    }

    #[test]
    fn old_forget_records_do_not_change_the_list() {
        // given: the root, an admin A and a member B; then the same entries plus a kind 5 record
        // about B written by the root
        let plain = vec![admit(ROOT, A, 10), admit(ROOT, B, 11)];
        let mut with_forget = plain.clone();
        with_forget.push(forget(ROOT, B));
        // when
        let (first, first_ignored) = tally_input(pid(ROOT), pid(ROOT), &plain);
        let (second, second_ignored) = tally_input(pid(ROOT), pid(ROOT), &with_forget);
        let (a, b) = (tally(&first), tally(&second));
        // then: the lists are equal, B is a member, and the old record is counted as ignored
        assert_eq!(a, b);
        assert!(b.is_member(&pid(B)));
        assert_eq!(second_ignored, first_ignored + 1);
    }
}

// --- setup and helpers ---

/// A network created by the node `ROOT`, seen by it.
fn setup() -> Scene {
    Scene::new(pid(ROOT))
}

/// One record of the scenario that is shuffled: the time in seconds, the author (the voter), the
/// subject; the nodes are seeds of `pid`.
#[derive(Clone, Copy, Debug)]
enum Step {
    Admit(u64, u8, u8),
    Grant(u64, u8, u8),
    Exclude(u64, u8, u8),
}

impl Step {
    fn write(&self, scene: &mut Scene) {
        match *self {
            Step::Admit(at, author, subject) => scene.admit(at, pid(author), pid(subject)),
            Step::Grant(at, author, subject) => scene.grant(at, pid(author), pid(subject)),
            Step::Exclude(at, voter, subject) => scene.exclude(at, pid(voter), pid(subject)),
        };
    }
}

fn ledger_of(steps: &[Step]) -> Ledger {
    let mut scene = setup();
    steps.iter().for_each(|s| s.write(&mut scene));
    scene.ledger()
}
