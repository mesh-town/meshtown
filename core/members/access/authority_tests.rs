//! Tests of the counting rules and the standing of nodes (`authority.rs`).

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::members::access::tally::{Exclusion, How};
use crate::transport::net::contract::pid;

struct Roll(BTreeSet<PeerId>);

impl Forgotten for Roll {
    fn forgotten(&self, p: &PeerId) -> bool {
        self.0.contains(p)
    }
}

fn excluded_of(p: PeerId) -> Excluded {
    let how = How::Votes;
    BTreeMap::from([(
        p,
        Exclusion {
            how,
            at_micros: 1,
            voters: vec![p],
        },
    )])
}

#[test]
fn peer_need_is_one_up_to_two_and_two_beyond() {
    // given the peer rules
    // when / then the threshold follows the number of active nodes
    assert!((0..=2).all(|n| Rules::Peer.need(n) == 1));
    assert_eq!(Rules::Peer.need(3), 2);
    assert_eq!(Rules::Peer.need(9), 2);
}

#[test]
fn peer_seeds_are_me_alone() {
    // given a node
    let me = pid(1);
    // when / then
    assert_eq!(Rules::Peer.seeds(me), BTreeSet::from([me]));
    assert!(!Rules::Peer.is_managed());
}

#[test]
fn witness_of_another_author_does_not_count() {
    // given
    let (me, other) = (pid(1), pid(2));
    // when / then
    assert!(!Rules::Peer.admit_counts(Via::Witness, &other, &me));
    assert!(Rules::Peer.admit_counts(Via::Witness, &me, &me));
}

#[test]
fn invite_none_and_migration_count_for_anyone() {
    // given
    let (me, other) = (pid(1), pid(2));
    // when / then
    for via in [Via::Invite, Via::None, Via::Migration] {
        assert!(Rules::Peer.admit_counts(via, &other, &me));
    }
}

#[test]
fn standing_tells_present_excluded_forgotten_and_stranger() {
    // given a closure of three, one excluded and one forgotten
    let (a, b, c, x) = (pid(1), pid(2), pid(3), pid(9));
    let closure = BTreeSet::from([a, b, c]);
    let excluded = excluded_of(b);
    let roll = Roll(BTreeSet::from([c]));
    // when
    let st = Standing::new(&closure, &excluded, &roll);
    // then
    assert!(st.present(&a) && st.counts(&a));
    assert!(!st.present(&b) && !st.counts(&b));
    assert!(st.present(&c) && !st.counts(&c));
    assert!(!st.present(&x) && !st.counts(&x));
}

#[test]
fn open_author_is_me_or_not_excluded() {
    // given me excluded and another node excluded
    let (me, b, c) = (pid(1), pid(2), pid(3));
    let closure = BTreeSet::from([me, b, c]);
    let mut excluded = excluded_of(me);
    excluded.extend(excluded_of(b));
    // when
    let st = Standing::new(&closure, &excluded, &());
    // then
    assert!(st.open(&me, &me) && st.backs(&me, &me));
    assert!(!st.open(&me, &b) && !st.backs(&me, &b));
    assert!(st.backs(&me, &c) && !st.backs(&me, &pid(9)));
}
