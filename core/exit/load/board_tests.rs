//! Tests of the overload record's format: parsing and reading of others' records.

use super::*;
use crate::transport::net::mem::{MemReplica, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::transport::net::{NodeSecret, ReplicaEntry};
use std::sync::Arc;

#[test]
fn parse_reads_known_reasons() {
    // given / when / then: the three words of this version
    assert_eq!(parse(b"limit"), Some(Cause::Known(Reason::Limit)));
    assert_eq!(parse(b"share"), Some(Cause::Known(Reason::Share)));
    assert_eq!(parse(b"memory"), Some(Cause::Known(Reason::Memory)));
}

#[test]
fn parse_unknown_word_is_other() {
    // given: a well-formed word a newer version may write
    // when / then: overloaded, cause other
    assert_eq!(parse(b"disk-full"), Some(Cause::Other));
}

#[test]
fn parse_ignores_tail() {
    // given: a known word with a tail
    // when / then: the tail changes nothing, even a strange one
    assert_eq!(
        parse(b"share 12 members"),
        Some(Cause::Known(Reason::Share))
    );
    assert_eq!(parse(b"limit Tail"), Some(Cause::Known(Reason::Limit)));
}

#[test]
fn parse_rejects_garbage_without_panic() {
    // given: empty, too long, not ASCII, capitals, a word that is too long, a leading space
    let long = [b'a'; 65];
    let word17 = [b'a'; 17];
    let cases: [&[u8]; 7] = [
        b"",
        &long,
        "caf\u{e9}".as_bytes(),
        b"Limit",
        &word17,
        b" limit",
        &[0xff, 0xfe],
    ];
    // when / then: none of them reads as overloaded
    for case in cases {
        assert_eq!(parse(case), None, "{case:?}");
    }
}

#[test]
fn parse_boundaries_of_format() {
    // given: values on the edges of the format
    let mut full = b"share ".to_vec();
    full.extend([b'x'; 58]);
    let word16 = [b'a'; 16];
    let mut word16_tail = word16.to_vec();
    word16_tail.extend(b" tail");
    // when / then: non-ASCII tail is garbage; exactly 64 bytes and a 16-byte word are fine
    assert_eq!(parse(b"limit \xff"), None);
    assert_eq!(full.len(), 64);
    assert_eq!(parse(&full), Some(Cause::Known(Reason::Share)));
    assert_eq!(parse(&word16), Some(Cause::Other));
    assert_eq!(parse(&word16_tail), Some(Cause::Other));
}

#[tokio::test]
async fn read_returns_live_readable_records_with_time() {
    // given: A has a live record, B a tombstone, C garbage, D a record under a longer key
    let rep = replica();
    let (a, b, c, d) = (peer(2), peer(3), peer(4), peer(5));
    rep.put_raw(entry(a, KEY, b"share"));
    rep.put_raw(entry(b, KEY, b""));
    rep.put_raw(entry(c, KEY, b"LIMIT"));
    rep.put_raw(entry(d, b"node/exit-load/x", b"share"));
    // when: we read the loads
    let got = read_exit_loads(&*rep).await.unwrap();
    // then: only A, with its time
    let expected = ExitLoad {
        cause: Cause::Known(Reason::Share),
        ts_micros: 1_000,
    };
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got.get(&a), Some(&expected));
}

fn replica() -> Arc<MemReplica> {
    let ctx = SyncCtx {
        secret: NodeSecret::from_bytes(&[1; 32]),
        network_seed: [9; 32],
        store_dir: None,
    };
    MemSyncNet::new()
        .backend(ctx)
        .replica_mem(REPLICA_NET)
        .unwrap()
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

fn entry(author: PeerId, key: &[u8], value: &'static [u8]) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key: key.to_vec(),
        value: Bytes::from_static(value),
        ts_micros: 1_000,
    }
}
