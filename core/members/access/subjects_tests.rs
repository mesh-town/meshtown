//! Tests of `my_votes_under`: which of the own exclusion votes still refuse a node.

use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;

use super::my_votes_under;
use crate::members::access::authority::Rules;
use crate::members::access::record::{encode, exclude_key, key_for, AclKind, AclValue, Via};
use crate::members::access::tally::{Exclusion, How};
use crate::transport::net::contract::pid;
use crate::transport::net::{PeerId, ReplicaEntry};

fn me() -> PeerId {
    pid(1)
}

fn managed() -> Rules {
    Rules::Managed { root: pid(2) }
}

#[test]
fn managed_keeps_only_votes_the_ledger_still_excludes() {
    // given: two live own votes, the ledger still excludes only the first subject
    let (p, q) = (pid(5), pid(6));
    let entries = vec![live(me(), p), live(me(), q)];
    // when
    let votes = my_votes_under(managed(), &entries, me(), &excluded(&[p]));
    // then: the vote against the node that is no longer excluded has dropped out
    assert_eq!(votes, ids(&[p]));
}

#[test]
fn others_votes_tombstones_and_me_never_count() {
    // given: a vote of another node, an own tombstone and an own vote against me; all subjects excluded
    let (p, q, other) = (pid(5), pid(6), pid(7));
    let entries = vec![live(other, p), withdrawn(me(), q), live(me(), me())];
    let ledger = excluded(&[p, q, me()]);
    for rules in [managed(), Rules::Peer] {
        // when
        let votes = my_votes_under(rules, &entries, me(), &ledger);
        // then
        assert_eq!(
            votes,
            ids(&[]),
            "no own live vote against another node under {rules:?}"
        );
    }
}

#[test]
fn peer_keeps_every_live_own_vote() {
    // given: two live own votes and a vote against me; the ledger excludes nobody
    let (p, q) = (pid(5), pid(6));
    let entries = vec![live(me(), p), live(me(), q), live(me(), me())];
    // when
    let votes = my_votes_under(Rules::Peer, &entries, me(), &BTreeMap::new());
    // then: the ledger does not matter in a peer network
    assert_eq!(votes, ids(&[p, q]));
}

/// A live exclusion vote of `author` against `subject`.
fn live(author: PeerId, subject: PeerId) -> ReplicaEntry {
    let value = AclValue {
        kind: AclKind::Exclude,
        subject,
        via: Via::Invite,
        made_unix: 1,
        note: String::new(),
    };
    ReplicaEntry {
        author,
        key: key_for(AclKind::Exclude, &subject),
        value: Bytes::from(encode(&value)),
        ts_micros: 1_000_000,
    }
}

/// The exclusion vote of `author` against `subject`, taken back.
fn withdrawn(author: PeerId, subject: PeerId) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key: exclude_key(&subject),
        value: Bytes::new(),
        ts_micros: 2_000_000,
    }
}

fn excluded(subjects: &[PeerId]) -> BTreeMap<PeerId, Exclusion> {
    let one = Exclusion {
        how: How::Votes,
        at_micros: 10,
        voters: vec![],
    };
    subjects.iter().map(|p| (*p, one.clone())).collect()
}

fn ids(subjects: &[PeerId]) -> BTreeSet<PeerId> {
    subjects.iter().copied().collect()
}
