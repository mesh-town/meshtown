//! Tests of reading the exit flag and a value by key (`node_state`) on an in-memory replica: the filters
//! by exact key and by tombstones live in the consumer, not in the storage. Live reconciliation between
//! nodes is checked by `tests/exit_flag.rs` and its neighbors.

use crate::net::mem::{MemReplica, MemSyncNet};
use crate::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::net::{ReplicaEntry, NodeSecret, PeerId};
use bytes::Bytes;
use std::sync::Arc;

use super::*;

const VALUE_KEY: &[u8] = b"node/test/";

mod exits {
    use super::*;

    #[tokio::test]
    async fn live_record_is_on() {
        // given: a foreign author announced an exit (a live record)
        let r = setup();
        let x = peer(2);
        r.rep.put_raw(entry(x, EXIT_KEY, EXIT_ON));
        // when: we read the exit flags
        let exits = read_exit_flags(&*r.rep).await.unwrap();
        // then: it has true
        assert_eq!(exits.get(&x), Some(&true));
    }

    #[tokio::test]
    async fn tombstone_is_off_not_absent() {
        // given: a foreign author turned the exit off (a tombstone — an empty value)
        let r = setup();
        let x = peer(2);
        r.rep.put_raw(entry(x, EXIT_KEY, b""));
        // when: we read the exit flags
        let exits = read_exit_flags(&*r.rep).await.unwrap();
        // then: it is in the map with false, not gone
        assert_eq!(exits.get(&x), Some(&false));
    }

    #[tokio::test]
    async fn longer_key_with_same_prefix_is_not_a_flag() {
        // given: an author wrote a key that merely starts with `node/exit/`
        let r = setup();
        let x = peer(2);
        r.rep.put_raw(entry(x, b"node/exit/extra", EXIT_ON));
        // when: we read the exit flags
        let exits = read_exit_flags(&*r.rep).await.unwrap();
        // then: no flag
        assert!(exits.is_empty(), "a foreign key was taken for the flag: {exits:?}");
    }
}

mod values {
    use super::*;

    #[tokio::test]
    async fn keeps_only_live_records_of_exact_key() {
        // given: the key has a live record of author X, a tombstone of author Y and a record Z under a longer key
        let r = setup();
        let (x, y, z) = (peer(2), peer(3), peer(4));
        r.rep.put_raw(entry(x, VALUE_KEY, b"v"));
        r.rep.put_raw(entry(y, VALUE_KEY, b""));
        r.rep.put_raw(entry(z, b"node/test/more", b"v"));
        // when: we read the key's values
        let got = read_values(&*r.rep, VALUE_KEY).await.unwrap();
        // then: only the live value of X
        assert_eq!(got.len(), 1, "extra authors: {got:?}");
        assert_eq!(got.get(&x), Some(&Bytes::from_static(b"v")));
    }
}

// --- setup and helpers ---

struct Rig {
    rep: Arc<MemReplica>,
}

/// A node with the author `[1; 32]` and an empty shared in-memory replica.
fn setup() -> Rig {
    let secret = NodeSecret::from_bytes(&[1; 32]);
    let ctx = SyncCtx {
        secret,
        network_seed: [9; 32],
        store_dir: None,
    };
    let backend = MemSyncNet::new().backend(ctx);
    Rig {
        rep: backend.replica_mem(REPLICA_NET).unwrap(),
    }
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

/// A record of the author `author`; an empty `value` is a tombstone.
fn entry(author: PeerId, key: &[u8], value: &'static [u8]) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key: key.to_vec(),
        value: Bytes::from_static(value),
        ts_micros: 1_000,
    }
}
