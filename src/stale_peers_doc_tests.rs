//! Test of reading entry stamps from an in-memory replica (`read_doc_seen`).

use crate::net::mem::{MemReplica, MemSyncNet};
use crate::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::net::{ReplicaEntry, NodeSecret, PeerId, Replica};
use bytes::Bytes;
use std::sync::Arc;

use super::*;

/// "Now" for reading, Unix seconds: certainly later than any stamps in these tests.
const NOW: u64 = 4_000_000_000;

#[tokio::test]
async fn read_doc_seen_sees_own_entry_and_tombstone() {
    let r = setup();
    let before = now_secs();
    r.rep
        .set_own(b"node/test/", Bytes::from_static(b"x"))
        .await
        .unwrap();
    r.rep.del_own(b"node/test/").await.unwrap();
    let after = now_secs();
    let seen = read_doc_seen(&*r.rep, after + 5).await.unwrap();
    let got = *seen.get(&r.me).expect("own entry is visible");
    assert!(
        got >= before && got <= after,
        "{before} <= {got} <= {after}"
    );
}

mod foreign_author {
    use super::*;

    #[tokio::test]
    async fn seen_at_entry_time_in_seconds() {
        // given: an entry of a foreign author with a time of 1000 s
        let r = setup();
        let x = peer(2);
        r.rep.put_raw(entry(x, b"node/a/", b"v", 1_000 * MICROS));
        // when: we read the stamps
        let seen = read_doc_seen(&*r.rep, NOW).await.unwrap();
        // then: the author has a stamp of 1000 s, there is no own entry, no phantom keys
        assert_eq!(seen.get(&x), Some(&1_000));
    assert_eq!(seen.len(), 1, "extra authors: {seen:?}");
    }

    #[tokio::test]
    async fn tombstone_counts_as_seen() {
        // given: only a tombstone (an empty value) came from the author, with a time of 2000 s
        let r = setup();
        let x = peer(2);
        r.rep.put_raw(entry(x, b"node/a/", b"", 2_000 * MICROS));
        // when: we read the stamps
        let seen = read_doc_seen(&*r.rep, NOW).await.unwrap();
        // then: a tombstone is a sign of life of the author at its time
        assert_eq!(seen.get(&x), Some(&2_000));
    }

    #[tokio::test]
    async fn newest_of_author_keys_wins() {
        // given: the author has three keys — 1000 s, 3000 s (a tombstone) and 2000 s
        let r = setup();
        let x = peer(2);
        r.rep.put_raw(entry(x, b"node/a/", b"v", 1_000 * MICROS));
        r.rep.put_raw(entry(x, b"node/b/", b"", 3_000 * MICROS));
        r.rep
            .put_raw(entry(x, b"list/c.com/", b"v", 2_000 * MICROS));
        // when: we read the stamps
        let seen = read_doc_seen(&*r.rep, NOW).await.unwrap();
        // then: the greatest stamp is taken
        assert_eq!(seen.get(&x), Some(&3_000));
    }

    #[tokio::test]
    async fn future_time_is_clamped_to_now() {
        // given: an entry with a time a hundred seconds later than "now"
        let r = setup();
        let x = peer(2);
        r.rep
            .put_raw(entry(x, b"node/a/", b"v", (NOW + 100) * MICROS));
        // when: we read the stamps with "now" = NOW
        let seen = read_doc_seen(&*r.rep, NOW).await.unwrap();
        // then: the stamp is clamped to NOW
        assert_eq!(seen.get(&x), Some(&NOW));
    }

    #[tokio::test]
    async fn authors_are_kept_apart() {
        // given: two authors with different stamps
        let r = setup();
        let (x, y) = (peer(2), peer(3));
        r.rep.put_raw(entry(x, b"node/a/", b"v", 1_000 * MICROS));
        r.rep.put_raw(entry(y, b"node/a/", b"v", 2_000 * MICROS));
        // when: we read the stamps
        let seen = read_doc_seen(&*r.rep, NOW).await.unwrap();
        // then: each has their own
        assert_eq!(seen.get(&x), Some(&1_000));
        assert_eq!(seen.get(&y), Some(&2_000));
    }
}

// --- setup and helpers ---

const MICROS: u64 = 1_000_000;

struct Rig {
    rep: Arc<MemReplica>,
    me: PeerId,
}

/// A node with the author `[3; 32]` and an empty shared in-memory replica.
fn setup() -> Rig {
    let secret = NodeSecret::from_bytes(&[3; 32]);
    let ctx = SyncCtx {
        secret: secret.clone(),
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = MemSyncNet::new().backend(ctx);
    let rep = backend.replica_mem(REPLICA_NET).unwrap();
    Rig {
        rep,
        me: secret.peer_id(),
    }
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

/// An entry of the author `author`; an empty `value` is a tombstone.
fn entry(author: PeerId, key: &[u8], value: &'static [u8], ts_micros: u64) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key: key.to_vec(),
        value: Bytes::from_static(value),
        ts_micros,
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
