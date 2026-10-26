//! The boot plan in a network with a root: an exclusion written by the root counts.

use std::collections::BTreeSet;

use bytes::Bytes;

use super::*;
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

fn plan_under(root: PeerId, snap: &[ReplicaEntry], me: PeerId, cached: &[PeerId]) -> BootPlan {
    let boot = AccessBoot {
        me,
        initial: Vec::new(),
        cached: cached.to_vec(),
        now_unix: NOW,
        root,
    };
    plan_admits(snap, &BTreeSet::new(), &boot)
}

#[test]
fn the_roots_exclusion_is_honoured() {
    let (root, me, x, y) = (pid(9), pid(1), pid(2), pid(3));
    let snap = excluded_by_root(root, x);
    let plan = plan_under(root, &snap, me, &[x, y]);
    assert_eq!(plan.witness, vec![y]);
}

#[test]
fn the_roots_exclusion_of_me_leaves_the_node() {
    let (root, me, x) = (pid(9), pid(1), pid(2));
    let snap = excluded_by_root(root, me);
    let plan = plan_under(root, &snap, me, &[x]);
    assert!(plan.me_left && plan.witness.is_empty());
}
