//! Tests of the roster round on `MemRecords`: writer merge, a foreign version, a race, a store error.

use super::*;
use crate::net::contract::pid;
use crate::net_hyperdht::mem::MemRecords;
use crate::net_mainline::record::{merge_keys, seal, Roster, RosterEntry};
use crate::network_key::NetworkKey;

const NOW: u64 = 1_760_000_000;
const MS: u64 = NOW * 1000;

fn keys() -> RosterKeys {
    RosterKeys::from_subkeys(&NetworkKey::from_bytes([0x44; 32]).subkeys())
}

fn ids(r: &Roster) -> Vec<[u8; 32]> {
    r.entries.iter().map(|e| e.id).collect()
}

fn id(seed: u8) -> [u8; 32] {
    *pid(seed).as_bytes()
}

async fn stored_ids(store: &MemRecords) -> Vec<[u8; 32]> {
    let (r, _) = read(store, &keys()).await.expect("read").expect("record exists");
    ids(&r)
}

#[test]
fn next_seq_takes_max_of_prev_plus_one_and_now() {
    assert_eq!(next_seq(None, 1000), 1000);
    assert_eq!(next_seq(Some(1005), 1000), 1006);
}

#[tokio::test]
async fn publish_round_on_empty_store_writes_self() {
    let store = MemRecords::new();
    publish_round(&store, &keys(), pid(1), NOW, MS).await.expect("round");
    assert_eq!(stored_ids(&store).await, vec![id(1)]);
}

#[tokio::test]
async fn second_writer_merges_first() {
    let store = MemRecords::new();
    publish_round(&store, &keys(), pid(1), NOW, MS).await.expect("A");
    publish_round(&store, &keys(), pid(2), NOW + 1, MS + 1000).await.expect("B");
    assert_eq!(stored_ids(&store).await, vec![id(2), id(1)]);
}

#[tokio::test]
async fn foreign_version_is_treated_as_empty() {
    let store = MemRecords::new();
    let old = Roster {
        version: 1,
        entries: vec![RosterEntry { id: id(9), relay: None, addrs: vec![], seen_unix: NOW }],
    };
    let sealed = seal(&keys().seal_key, &old, [7; 12]).expect("seal");
    store.put(&keys().record_key, sealed, 1).await.expect("put");
    publish_round(&store, &keys(), pid(1), NOW, MS).await.expect("round");
    assert_eq!(stored_ids(&store).await, vec![id(1)]);
}

#[tokio::test]
async fn undecryptable_record_is_overwritten() {
    let store = MemRecords::new();
    let other = merge_keys(None, id(9), NOW);
    let sealed = seal(&[0x99; 32], &other, [7; 12]).expect("seal");
    store.put(&keys().record_key, sealed, 1).await.expect("put");
    publish_round(&store, &keys(), pid(1), NOW, MS).await.expect("round");
    assert_eq!(stored_ids(&store).await, vec![id(1)]);
}

#[tokio::test]
async fn concurrent_writers_converge() {
    let store = MemRecords::new();
    let k = keys();
    publish_round(&store, &k, pid(3), NOW, MS).await.expect("C");
    // A and B read the same prev; they write in turn, B with a greater seq overwrites A
    let (prev, _) = read(&store, &k).await.expect("read").expect("exists");
    for (seed, ms) in [(1u8, MS + 10), (2u8, MS + 20)] {
        let merged = merge_keys(Some(prev.clone()), id(seed), NOW + 1);
        let sealed = seal(&k.seal_key, &merged, [seed; 12]).expect("seal");
        store.put(&k.record_key, sealed, ms).await.expect("put");
    }
    assert_eq!(stored_ids(&store).await, vec![id(2), id(3)]);
    publish_round(&store, &k, pid(1), NOW + 2, MS + 2000).await.expect("A");
    publish_round(&store, &k, pid(2), NOW + 3, MS + 3000).await.expect("B");
    let got = stored_ids(&store).await;
    for seed in [1, 2, 3] {
        assert!(got.contains(&id(seed)), "{seed} is not in the record");
    }
}

#[tokio::test]
async fn store_error_is_returned() {
    let store = MemRecords::new();
    store.fail_next(1);
    let got = publish_round(&store, &keys(), pid(1), NOW, MS).await;
    assert!(got.is_err());
}
