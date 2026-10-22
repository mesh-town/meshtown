//! Tests for the replica core (`replica.rs`): tombstones, `live`, the short replica id.

use crate::transport::net::contract::{pid, SEED_A};
use crate::transport::net::replica::{live, ReplicaEntry, ReplicaId};
use bytes::Bytes;

mod tombstone {
    use super::*;

    #[test]
    fn is_tombstone_is_empty_value() {
        // given: a record with an empty value and a record with a non-empty one
        let empty = entry(b"k", b"");
        let full = entry(b"k", b"v");
        // when/then: a tombstone is only an empty value
        assert!(empty.is_tombstone(), "an empty value is a tombstone");
        assert!(!full.is_tombstone(), "a non-empty value is not a tombstone");
    }
}

mod live_filter {
    use super::*;

    #[test]
    fn live_drops_tombstones_keeps_order() {
        // given: five records, the second and the fourth are tombstones
        let entries = vec![
            entry(b"a", b"1"),
            entry(b"b", b""),
            entry(b"c", b"3"),
            entry(b"d", b""),
            entry(b"e", b"5"),
        ];
        // when: we keep the live ones
        let kept = live(entries);
        // then: no tombstones, the order of the rest is unchanged
        let keys: Vec<Vec<u8>> = kept.iter().map(|e| e.key.clone()).collect();
        assert_eq!(keys, vec![b"a".to_vec(), b"c".to_vec(), b"e".to_vec()]);
        assert!(
            kept.iter().all(|e| !e.is_tombstone()),
            "a tombstone in the result"
        );
    }
}

mod replica_id {
    use super::*;

    #[test]
    fn replica_id_short_is_ten_hex() {
        // given: an id of identical 0xab bytes
        let id = ReplicaId([0xab; 32]);
        // when/then: the short form is the first ten hex digits
        assert_eq!(id.fmt_short(), "ababababab");
    }
}

fn entry(key: &[u8], value: &[u8]) -> ReplicaEntry {
    ReplicaEntry {
        author: pid(SEED_A),
        key: key.to_vec(),
        value: Bytes::copy_from_slice(value),
        ts_micros: 1,
    }
}
