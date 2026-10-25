//! The boot plan under the counting rules of the network: an exclusion written by the root counts in a managed
//! network and is ignored (the root is not an admitted author) under peer rules.

use std::collections::BTreeSet;

use bytes::Bytes;

use super::*;
use crate::members::access::authority::Rules;
use crate::members::access::record::{encode, key_for, AclKind, AclValue, Via};
use crate::transport::net::contract::pid;
use crate::transport::net::{PeerId, ReplicaEntry};

const NOW: u64 = 1_792_400_000;

fn record(author: PeerId, kind: AclKind, subject: PeerId, ts_micros: u64) -> ReplicaEntry {
    let value = AclValue {
        kind,
        subject,
        via: Via::Invite,
        made_unix: NOW,
        note: String::new(),
    };
    ReplicaEntry {
        author,
        key: key_for(kind, &subject),
        value: Bytes::from(encode(&value)),
        ts_micros,
    }
}

/// The root lets `subject` in and later excludes it.
fn excluded_by_root(root: PeerId, subject: PeerId) -> Vec<ReplicaEntry> {
    vec![
        record(root, AclKind::Admit, subject, 10),
        record(root, AclKind::Exclude, subject, 20),
    ]
}

fn plan_under(rules: Rules, snap: &[ReplicaEntry], me: PeerId, cached: &[PeerId]) -> BootPlan {
    let boot = AccessBoot {
        me,
        initial: Vec::new(),
        cached: cached.to_vec(),
        now_unix: NOW,
        rules,
    };
    plan_admits(snap, &BTreeSet::new(), &boot)
}

#[test]
fn managed_rules_honour_the_roots_exclusion() {
    let (root, me, x, y) = (pid(9), pid(1), pid(2), pid(3));
    let snap = excluded_by_root(root, x);
    let plan = plan_under(Rules::Managed { root }, &snap, me, &[x, y]);
    assert_eq!(plan.witness, vec![y]);
}

#[test]
fn peer_rules_ignore_the_roots_exclusion() {
    let (root, me, x, y) = (pid(9), pid(1), pid(2), pid(3));
    let snap = excluded_by_root(root, x);
    let plan = plan_under(Rules::Peer, &snap, me, &[x, y]);
    assert_eq!(plan.witness, vec![x, y]);
}

#[test]
fn managed_root_exclusion_of_me_leaves_the_node() {
    let (root, me, x) = (pid(9), pid(1), pid(2));
    let snap = excluded_by_root(root, me);
    let managed = plan_under(Rules::Managed { root }, &snap, me, &[x]);
    assert!(managed.me_left && managed.witness.is_empty());
    assert!(!plan_under(Rules::Peer, &snap, me, &[x]).me_left);
}
