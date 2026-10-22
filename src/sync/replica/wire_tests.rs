//! Tests for the replica wire: slicing into batches and the broadcast body.

use super::*;
use crate::net::id::NodeSecret;
use crate::sync::replica::entry::seal;

fn rid() -> ReplicaId {
    ReplicaId([1; 32])
}

fn put(key: &[u8], value: &[u8], ts: u64) -> WireEntry {
    seal(&NodeSecret::from_bytes(&[2; 32]), &rid(), key, value, ts)
}

mod batches {
    use super::*;

    #[test]
    fn each_under_limit_and_order_kept() {
        let all: Vec<WireEntry> = (0..300u64)
            .map(|i| put(format!("k{i:04}").as_bytes(), &[7u8; 4096], i))
            .collect();
        let cut = batches(all.clone());
        assert!(cut.len() > 1);
        for b in &cut {
            let size = postcard::to_stdvec(&Frame::Entries(b.clone()))
                .unwrap()
                .len();
            assert!(size <= MAX_BATCH_BYTES, "batch of {size} bytes");
        }
        let flat: Vec<WireEntry> = cut.into_iter().flatten().collect();
        assert_eq!(flat, all);
    }

    #[test]
    fn empty_gives_no_batches() {
        assert!(batches(Vec::new()).is_empty());
    }
}

mod push {
    use super::*;

    #[test]
    fn roundtrip() {
        let entries = vec![put(b"a", b"1", 1), put(b"b", b"2", 2)];
        let body = encode_push(&rid(), entries.clone()).unwrap();
        let got = decode_push(&body).unwrap();
        assert_eq!(got.version, PROTO_VERSION);
        assert_eq!(got.replica, rid().0);
        assert_eq!(got.entries, entries);
    }

    #[test]
    fn garbage_is_error() {
        assert!(decode_push(&[0xff, 0xff, 0xff, 0xff, 0xff]).is_err());
        assert!(decode_push(&[]).is_err());
    }
}
