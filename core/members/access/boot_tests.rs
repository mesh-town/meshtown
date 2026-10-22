//! Tests of the boot of the membership list (`boot.rs`): the class of the node (long-time or newcomer), the
//! migration of recent authors and the boot marker. The rules about whom never to
//! admit, the ceiling and the log line are in `boot_rules_tests.rs`.
//! Nodes: `me` is the own node (seed 1), the others are named by what they do in the test.

use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;

use super::*;
use crate::access::record::{admit_key, parse_entry, put, AclKind, AclValue, Parsed, Via, BOOT_KEY};
use crate::net::contract::{pid, secret};
use crate::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::net::{PeerId, Replica, ReplicaEntry};

#[path = "boot_rules_tests.rs"]
mod rules;

/// The moment of the boot in most tests (unix seconds).
const NOW: u64 = 1_792_400_000;
const DAY: u64 = 86_400;
const MICROS: u64 = 1_000_000;

mod first_start {
    use super::*;

    mod long_time_node {
        use super::*;

        #[tokio::test]
        async fn legacy_admits_recent_authors_and_cached() {
            // given: a node that wrote its exit record before, an author active 29 days ago and a cached neighbour
            let rig = setup();
            let (author, cached) = (pid(2), pid(3));
            rig.given_prior_life().await;
            rig.given_record_of(author, b"node/exit/a", ago(29 * DAY));
            // when: the first boot of the new version
            let out = rig.when_boot(&[], &[cached], NOW).await;
            // then: the cached one is witnessed and then promoted to a vote, the author is migrated
            let want = admits_of(&[(cached, Via::None), (author, Via::Migration)]);
            assert_eq!(rig.admits().await, want, "the admits written by the node");
            assert_eq!(out, 2);
        }

        #[tokio::test]
        async fn legacy_marks_the_boot_with_migration() {
            // given: a node with a prior life
            let rig = setup();
            rig.given_prior_life().await;
            // when: the first boot
            rig.when_boot(&[], &[], NOW).await;
            // then: the marker says "long-time node, made now", and is the node's own
            let marker = rig.marker().await.expect("the boot marker must be written");
            assert_eq!(marker.via, Via::Migration, "the class is kept in the marker");
            assert_eq!(marker.made_unix, NOW);
            assert_eq!(marker.subject, rig.me, "the marker names the node itself");
        }

        #[tokio::test]
        async fn legacy_skips_authors_older_than_30_days() {
            // given: one author last seen 31 days ago, another one a day ago
            let rig = setup();
            let (old, recent) = (pid(2), pid(3));
            rig.given_prior_life().await;
            rig.given_record_of(old, b"node/exit/a", ago(31 * DAY));
            rig.given_record_of(recent, b"node/exit/b", ago(DAY));
            // when: the first boot
            rig.when_boot(&[], &[], NOW).await;
            // then: only the recent one is admitted
            assert_eq!(rig.admits().await, admits_of(&[(recent, Via::Migration)]));
        }

        #[tokio::test]
        async fn lookback_includes_30_days_back_and_excludes_marker_time() {
            // given: authors at the edges of [made - 30 days, made)
            let rig = setup();
            let (oldest_in, just_out, at_made, just_before_made) = (pid(2), pid(3), pid(4), pid(5));
            let from = NOW * MICROS - 30 * DAY * MICROS;
            rig.given_prior_life().await;
            rig.given_record_of(oldest_in, b"k/1", from);
            rig.given_record_of(just_out, b"k/2", from - 1);
            rig.given_record_of(at_made, b"k/3", NOW * MICROS);
            rig.given_record_of(just_before_made, b"k/4", NOW * MICROS - 1);
            // when: the first boot
            rig.when_boot(&[], &[], NOW).await;
            // then: the lower edge is in, the upper edge is out
            let want = admits_of(&[(oldest_in, Via::Migration), (just_before_made, Via::Migration)]);
            assert_eq!(rig.admits().await, want);
        }

        #[tokio::test]
        async fn migration_counts_any_key_of_the_author_tombstones_included() {
            // given: an author whose only recent records are an `acl/` record and a tombstone
            let rig = setup();
            let (by_acl, by_tombstone) = (pid(2), pid(3));
            rig.given_prior_life().await;
            rig.given_record_of(by_acl, &admit_key(&pid(9)), ago(DAY));
            rig.given_tombstone_of(by_tombstone, b"node/exit/b", ago(2 * DAY));
            // when: the first boot
            rig.when_boot(&[], &[], NOW).await;
            // then: both are migrated
            let want = admits_of(&[(by_acl, Via::Migration), (by_tombstone, Via::Migration)]);
            assert_eq!(rig.admits().await, want);
        }

        #[tokio::test]
        async fn legacy_class_counts_own_tombstone() {
            // given: the node's only record outside `acl/` is a tombstone (an exit that was switched off)
            let rig = setup();
            rig.given_prior_life().await;
            rig.shared.del_own(PRIOR_KEY).await.unwrap();
            // when: the first boot
            rig.when_boot(&[], &[], NOW).await;
            // then: it is still a long-time node: the marker says so
            assert_eq!(rig.marker().await.map(|m| m.via), Some(Via::Migration));
        }
    }

    mod newcomer {
        use super::*;

        #[tokio::test]
        async fn fresh_node_admits_only_initial_and_cached() {
            // given: a node with no records of its own; fresh authors are in the replica
            let rig = setup();
            let (initial, cached, stranger) = (pid(2), pid(3), pid(4));
            rig.given_record_of(stranger, b"node/exit/s", ago(60));
            // when: the first boot
            let out = rig.when_boot(&[initial], &[cached], NOW).await;
            // then: the two known neighbours are witnessed, the author is not
            let want = admits_of(&[(initial, Via::Witness), (cached, Via::Witness)]);
            assert_eq!(rig.admits().await, want);
            assert_eq!(out, 2);
        }

        #[tokio::test]
        async fn fresh_node_marks_the_boot_without_migration() {
            // given: a node with no records of its own
            let rig = setup();
            // when: the first boot
            rig.when_boot(&[], &[], NOW).await;
            // then: the marker exists and does not claim a prior life
            let marker = rig.marker().await.expect("the boot marker must be written");
            assert_eq!(marker.via, Via::None, "the class of a newcomer");
            assert_eq!(marker.made_unix, NOW);
        }

        #[tokio::test]
        async fn own_acl_records_do_not_make_legacy() {
            // given: the node's only records are list records (a new version wrote them), and a fresh author
            let rig = setup();
            let (admitted_before, author) = (pid(2), pid(3));
            rig.given_own_admit(admitted_before, Via::Invite).await;
            rig.given_record_of(author, b"node/exit/a", ago(DAY));
            // when: the first boot
            rig.when_boot(&[], &[], NOW).await;
            // then: it is a newcomer: nothing is migrated
            assert_eq!(rig.admits().await, admits_of(&[(admitted_before, Via::Invite)]));
            assert_eq!(rig.marker().await.map(|m| m.via), Some(Via::None));
        }

        #[tokio::test]
        async fn peer_in_initial_and_cached_is_admitted_once() {
            // given: one neighbour named both as initial and as cached
            let rig = setup();
            let both = pid(2);
            // when: the first boot
            let out = rig.when_boot(&[both], &[both], NOW).await;
            // then: one record
            assert_eq!(rig.admits().await, admits_of(&[(both, Via::Witness)]));
            assert_eq!(out, 1);
        }
    }
}

mod marker {
    use super::*;

    #[tokio::test]
    async fn done_boot_does_not_migrate_again() {
        // given: a first boot of a long-time node has migrated author A
        let rig = setup();
        let (a, b) = (pid(2), pid(3));
        rig.given_prior_life().await;
        rig.given_record_of(a, b"node/exit/a", ago(2 * DAY));
        rig.when_boot(&[], &[], NOW).await;
        // and: author B, also recent by his clock, arrives by synchronisation later
        rig.given_record_of(b, b"node/exit/b", ago(DAY));
        // when: the node starts again
        let out = rig.when_boot(&[], &[], NOW + 3600).await;
        // then: nothing is written
        assert_eq!(out, 0);
        assert_eq!(rig.admits().await, admits_of(&[(a, Via::Migration)]), "B is not migrated");
    }

    #[tokio::test]
    async fn done_boot_keeps_the_marker_as_it_was() {
        // given: a marker made at T
        let rig = setup();
        rig.given_marker(Via::Migration, NOW).await;
        // when: the boot runs a day later
        rig.when_boot(&[], &[], NOW + DAY).await;
        // then: the marker keeps its time
        assert_eq!(rig.marker().await.map(|m| m.made_unix), Some(NOW));
    }

    #[tokio::test]
    async fn unreadable_marker_means_done() {
        // given: a node with a prior life, a recent author, and a boot marker of a version nobody knows
        let rig = setup();
        let (author, cached) = (pid(2), pid(3));
        rig.given_prior_life().await;
        rig.given_record_of(author, b"node/exit/a", ago(DAY));
        let unknown = rig.given_raw_marker(&[9, 1, 2, 3]);
        // when: the node boots
        rig.when_boot(&[], &[cached], NOW).await;
        // then: no migration; the neighbour is still witnessed; the marker is untouched
        assert_eq!(rig.admits().await, admits_of(&[(cached, Via::Witness)]));
        assert_eq!(rig.shared.get(rig.me, BOOT_KEY).await.unwrap(), Some(unknown), "the marker was rewritten");
    }
}

#[path = "boot_order_tests.rs"]
mod order;

#[path = "boot_promote_tests.rs"]
mod promote;

// --- setup and helpers ---

/// Key of the record that makes a node "long-time": written by the previous version.
const PRIOR_KEY: &[u8] = b"node/exit/main";

struct Rig {
    shared: Arc<MemReplica>,
    own: Arc<MemReplica>,
    me: PeerId,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

/// A node (seed 1) with empty shared and own in-memory replicas.
fn setup() -> Rig {
    let net = MemSyncNet::new();
    let node = secret(1);
    let ctx = SyncCtx { secret: node.clone(), network_seed: [9; 32], store_dir: None };
    let backend = net.backend(ctx);
    let (shared, own) = (backend.replica_mem(REPLICA_NET).unwrap(), backend.replica_mem(REPLICA_DOMAINS).unwrap());
    Rig { shared, own, me: node.peer_id(), _keep: (net, backend) }
}

/// A time `secs` before `NOW`, in microseconds.
fn ago(secs: u64) -> u64 { (NOW - secs) * MICROS }

fn admits_of(rows: &[(PeerId, Via)]) -> BTreeMap<PeerId, Via> { rows.iter().copied().collect() }

impl Rig {
    /// The previous version wrote the exit record of this node.
    async fn given_prior_life(&self) { self.shared.set_own(PRIOR_KEY, Bytes::from_static(b"on")).await.unwrap(); }

    /// A record of another author with the time of his clock.
    fn given_record_of(&self, author: PeerId, key: &[u8], ts_micros: u64) {
        let value = Bytes::from_static(b"x");
        self.shared.put_raw(ReplicaEntry { author, key: key.to_vec(), value, ts_micros });
    }

    /// A tombstone of another author.
    fn given_tombstone_of(&self, author: PeerId, key: &[u8], ts_micros: u64) {
        self.shared.put_raw(ReplicaEntry { author, key: key.to_vec(), value: Bytes::new(), ts_micros });
    }

    async fn given_own_admit(&self, subject: PeerId, via: Via) {
        let value = AclValue { kind: AclKind::Admit, subject, via, made_unix: 111, note: String::new() };
        put(&*self.shared, &value).await.unwrap();
    }

    async fn given_marker(&self, via: Via, made_unix: u64) {
        let value = AclValue { kind: AclKind::Boot, subject: self.me, via, made_unix, note: String::new() };
        put(&*self.shared, &value).await.unwrap();
    }

    /// An own marker with arbitrary bytes; returns the entry as stored.
    fn given_raw_marker(&self, bytes: &'static [u8]) -> ReplicaEntry {
        let entry = ReplicaEntry { author: self.me, key: BOOT_KEY.to_vec(), value: Bytes::from_static(bytes), ts_micros: 5 };
        self.shared.put_raw(entry.clone());
        entry
    }

    async fn when_boot(&self, initial: &[PeerId], cached: &[PeerId], now_unix: u64) -> usize {
        let boot = AccessBoot { me: self.me, initial: initial.to_vec(), cached: cached.to_vec(), now_unix };
        run_boot(&*self.shared, &*self.own, &boot).await.expect("the boot must not fail on a healthy replica")
    }

    /// Subject and `via` of every live admit the node wrote.
    async fn admits(&self) -> BTreeMap<PeerId, Via> {
        let all = self.shared.scan(b"acl/admit/").await.unwrap();
        let mine = all.iter().filter(|e| e.author == self.me && !e.is_tombstone());
        mine.filter_map(|e| match parse_entry(e) {
            Parsed::Record { value, .. } => Some((value.subject, value.via)),
            _ => None,
        })
        .collect()
    }

    /// The own boot marker, if it is written and readable.
    async fn marker(&self) -> Option<AclValue> {
        let entry = self.shared.get(self.me, BOOT_KEY).await.unwrap()?;
        match parse_entry(&entry) {
            Parsed::Record { value, .. } => Some(value),
            _ => None,
        }
    }
}
