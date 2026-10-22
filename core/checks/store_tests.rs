//! Tests of the check record: serialization and the absence of the domain inside it (`store.rs`).

use super::*;
use crate::checks::check_key;
use crate::net::mem::{MemReplica, MemSyncNet};
use crate::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::net::{ReplicaEntry, NodeSecret, Replica};
use crate::reachability::Steps;
use bytes::Bytes;
use std::sync::Arc;

fn sample() -> CheckRecord {
    let steps = Steps {
        dns_ok: true,
        tcp: reachability::TcpStep::Ok,
        tls: reachability::TlsStep::Ok,
        http_status: 200,
        bytes_received: 4096,
        ..Steps::default()
    };
    let (verdict, reason) = reachability::classify(&steps);
    CheckRecord {
        v: CHECK_RECORD_VERSION,
        verdict,
        dns_ms: 30,
        tcp_ms: 40,
        tls_ms: 90,
        http_ms: 120,
        status: 200,
        bytes: 4096,
        stalled: false,
        at: 1_759_000_000,
        reason,
    }
}

#[test]
fn record_postcard_roundtrip() {
    let record = sample();
    let bytes = postcard::to_stdvec(&record).unwrap();
    let back: CheckRecord = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(back, record);
    assert!(bytes.len() < 200, "record is {} bytes", bytes.len());
}

#[test]
fn record_does_not_contain_domain() {
    let record = sample();
    let bytes = postcard::to_stdvec(&record).unwrap();
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert!(!text.contains("example.com"), "{text}");
    assert!(!text.contains("93.184"), "{text}");
}

mod result_of_entry {
    use super::*;

    #[tokio::test]
    async fn live_entry_gives_the_record() {
        // given: the replica holds a check record of the current version
        let r = setup();
        let entry = r.stored(&postcard::to_stdvec(&sample()).unwrap()).await;
        // when: we take the check result from the record
        // then: it is the one that was written
        assert_eq!(record_of(&entry), Some(sample()));
    }

    #[tokio::test]
    async fn tombstone_has_no_record() {
        // given: the check record is removed — the replica holds a tombstone
        let r = setup();
        let entry = r.tombstoned().await;
        assert!(entry.is_tombstone(), "the tombstone should remain readable");
        // when: we take the check result from the record
        // then: there is no result
        assert_eq!(record_of(&entry), None);
    }

    #[tokio::test]
    async fn foreign_version_has_no_record() {
        // given: a check record with a format version we don't know
        let r = setup();
        let future = CheckRecord {
            v: CHECK_RECORD_VERSION + 1,
            ..sample()
        };
        let entry = r.stored(&postcard::to_stdvec(&future).unwrap()).await;
        // when: we take the check result from the record
        // then: there is no result
        assert_eq!(record_of(&entry), None);
    }

    #[tokio::test]
    async fn garbage_bytes_have_no_record() {
        // given: the record holds bytes that don't parse as a check result
        let r = setup();
        let entry = r.stored(&[0xFF; 3]).await;
        // when: we take the check result from the record
        // then: there is no result
        assert_eq!(record_of(&entry), None);
    }
}

// --- setup and helpers ---

struct Rig {
    rep: Arc<MemReplica>,
}

/// A node with author `[3; 32]` and an empty shared in-memory replica.
fn setup() -> Rig {
    let ctx = SyncCtx {
        secret: NodeSecret::from_bytes(&[3; 32]),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = MemSyncNet::new().backend(ctx);
    Rig {
        rep: backend.replica_mem(REPLICA_NET).unwrap(),
    }
}

impl Rig {
    /// Puts a value under a domain's check key and returns the entry as reading returns it.
    async fn stored(&self, value: &[u8]) -> ReplicaEntry {
        let key = check_key("example.com");
        self.rep
            .set_own(&key, Bytes::copy_from_slice(value))
            .await
            .unwrap();
        self.only_entry().await
    }

    /// Puts an entry and removes it; returns the tombstone.
    async fn tombstoned(&self) -> ReplicaEntry {
        self.stored(b"x").await;
        let key = check_key("example.com");
        self.rep.del_own(&key).await.unwrap();
        self.only_entry().await
    }

    async fn only_entry(&self) -> ReplicaEntry {
        let mut all = self.rep.scan(CHECK_PREFIX.as_bytes()).await.unwrap();
        assert_eq!(all.len(), 1, "the replica should hold exactly one entry: {all:?}");
        all.remove(0)
    }
}
