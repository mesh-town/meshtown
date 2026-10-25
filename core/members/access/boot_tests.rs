//! Tests of the boot of the membership list (`boot.rs`): the class of the node and the boot marker. The rules about whom never to
//! admit, the ceiling and the log line are in `boot_rules_tests.rs`.
//! Nodes: `me` is the own node (seed 1), the others are named by what they do in the test.

use std::collections::BTreeMap;
use std::sync::Arc;

use bytes::Bytes;

use super::*;
use crate::members::access::record::{
    admit_key, parse_entry, put, AclKind, AclValue, Parsed, Via, BOOT_KEY,
};
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemReplica, MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{PeerId, Replica, ReplicaEntry};

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
        async fn a_node_with_a_prior_life_starts_as_a_new_one() {
            // given: a node with records of its own outside `acl/`, no marker, an author active a day ago
            let rig = setup();
            let (author, cached) = (pid(2), pid(3));
            rig.given_prior_life().await;
            rig.given_record_of(author, b"node/exit/a", ago(DAY));
            // when: the first boot
            let out = rig.when_boot(&[], &[cached], NOW).await;
            // then: the marker with the flag is written, only the witness is admitted, nothing is promoted
            let marker = rig.marker().await.expect("the boot marker must be written");
            assert_eq!(marker.via, Via::None, "the class of a newcomer");
            assert_eq!(rig.admits().await, admits_of(&[(cached, Via::Witness)]));
            assert_eq!(out, 1);
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
        async fn fresh_node_marks_the_boot() {
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
        async fn own_acl_records_keep_it_a_newcomer() {
            // given: the node's only records are list records (a new version wrote them), and a fresh author
            let rig = setup();
            let (admitted_before, author) = (pid(2), pid(3));
            rig.given_own_admit(admitted_before, Via::Invite).await;
            rig.given_record_of(author, b"node/exit/a", ago(DAY));
            // when: the first boot
            rig.when_boot(&[], &[], NOW).await;
            // then: it is a newcomer
            assert_eq!(
                rig.admits().await,
                admits_of(&[(admitted_before, Via::Invite)])
            );
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
        // then: the neighbour is still witnessed; the marker is untouched
        assert_eq!(rig.admits().await, admits_of(&[(cached, Via::Witness)]));
        assert_eq!(
            rig.shared.get(rig.me, BOOT_KEY).await.unwrap(),
            Some(unknown),
            "the marker was rewritten"
        );
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
    let ctx = SyncCtx {
        secret: node.clone(),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = net.backend(ctx);
    let (shared, own) = (
        backend.replica_mem(REPLICA_NET).unwrap(),
        backend.replica_mem(REPLICA_DOMAINS).unwrap(),
    );
    Rig {
        shared,
        own,
        me: node.peer_id(),
        _keep: (net, backend),
    }
}

/// A time `secs` before `NOW`, in microseconds.
fn ago(secs: u64) -> u64 {
    (NOW - secs) * MICROS
}

fn admits_of(rows: &[(PeerId, Via)]) -> BTreeMap<PeerId, Via> {
    rows.iter().copied().collect()
}

impl Rig {
    /// The previous version wrote the exit record of this node.
    async fn given_prior_life(&self) {
        self.shared
            .set_own(PRIOR_KEY, Bytes::from_static(b"on"))
            .await
            .unwrap();
    }

    /// A record of another author with the time of his clock.
    fn given_record_of(&self, author: PeerId, key: &[u8], ts_micros: u64) {
        let value = Bytes::from_static(b"x");
        self.shared.put_raw(ReplicaEntry {
            author,
            key: key.to_vec(),
            value,
            ts_micros,
        });
    }

    async fn given_own_admit(&self, subject: PeerId, via: Via) {
        let value = AclValue {
            kind: AclKind::Admit,
            subject,
            via,
            made_unix: 111,
            note: String::new(),
        };
        put(&*self.shared, &value).await.unwrap();
    }

    async fn given_marker(&self, via: Via, made_unix: u64) {
        let value = AclValue {
            kind: AclKind::Boot,
            subject: self.me,
            via,
            made_unix,
            note: String::new(),
        };
        put(&*self.shared, &value).await.unwrap();
    }

    /// An own marker with arbitrary bytes; returns the entry as stored.
    fn given_raw_marker(&self, bytes: &'static [u8]) -> ReplicaEntry {
        let entry = ReplicaEntry {
            author: self.me,
            key: BOOT_KEY.to_vec(),
            value: Bytes::from_static(bytes),
            ts_micros: 5,
        };
        self.shared.put_raw(entry.clone());
        entry
    }

    async fn when_boot(&self, initial: &[PeerId], cached: &[PeerId], now_unix: u64) -> usize {
        let boot = AccessBoot {
            me: self.me,
            initial: initial.to_vec(),
            cached: cached.to_vec(),
            now_unix,
            rules: Rules::Peer,
        };
        run_boot(&*self.shared, &*self.own, &boot)
            .await
            .expect("the boot must not fail on a healthy replica")
    }

    /// Subject and `via` of every live admit the node wrote.
    async fn admits(&self) -> BTreeMap<PeerId, Via> {
        let all = self.shared.scan(b"acl/admit/").await.unwrap();
        let mine = all
            .iter()
            .filter(|e| e.author == self.me && !e.is_tombstone());
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
