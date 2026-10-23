//! Cases of the replica contract (`replica_contract!`): helpers and local cases —
//! one replica of one node. The two-node cases (reconciliation) are in `replica_sync_cases`.
//! The cases are plain functions: the compiler checks their types even without a plugged-in suite.

use crate::transport::net::contract::{must_finish, pid, SyncPair, SEED_A, SEED_B, STEP};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::replica::{live, Replica, ReplicaEntry, ReplicaEvent, MAX_VALUE_BYTES};
use crate::transport::net::sync_backend::{SyncBackend, REPLICA_DOMAINS, REPLICA_NET};
use bytes::Bytes;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

pub(crate) type Rep = Arc<dyn Replica>;

/// Case keys: none is a prefix of another (`del_own` deletes by prefix).
pub(crate) const K1: &[u8] = b"k1/";
pub(crate) const K2: &[u8] = b"k2/";
pub(crate) const K3: &[u8] = b"k3/";

/// "No events" window, poll period and pause between writes of different authors (time is by the writer's clock).
pub(crate) const QUIET: Duration = Duration::from_millis(300);
pub(crate) const POLL: Duration = Duration::from_millis(20);
pub(crate) const PAUSE: Duration = Duration::from_millis(10);

pub(crate) async fn set_get_roundtrip(pair: SyncPair) {
    // given: replica "net" of node A
    let r = open_net(&pair.a).await;
    // when: A writes its own entry
    put(&r, K1, "v1").await;
    // then: get(me, key) returns it: the author is A, the key and value are the same
    assert_eq!(
        r.me(),
        pid(SEED_A),
        "me() is not the id of the environment key"
    );
    let got = get(&r, r.me(), K1).await.expect("no entry");
    assert_eq!(
        (got.author, got.key, got.value),
        (pid(SEED_A), K1.to_vec(), val("v1"))
    );
}

pub(crate) async fn overwrite_keeps_one_entry(pair: SyncPair) {
    // given: A wrote the key with the value v1
    let r = open_net(&pair.a).await;
    put(&r, K1, "v1").await;
    // when: A writes the same key with the value v2
    put(&r, K1, "v2").await;
    // then: the author has one entry per key, with the new value
    let all = scan(&r, b"").await;
    assert_eq!(all.len(), 1, "overwrite duplicates the entry: {all:?}");
    assert_eq!(all[0].value, val("v2"));
}

pub(crate) async fn empty_value_rejected(pair: SyncPair) {
    // given: A wrote K1 with the value v1
    let r = open_net(&pair.a).await;
    put(&r, K1, "v1").await;
    // when: A writes an empty value to K1 and to a new K2
    expect_config(
        "set_own(K1, empty)",
        must_finish(r.set_own(K1, Bytes::new())).await,
    );
    expect_config(
        "set_own(K2, empty)",
        must_finish(r.set_own(K2, Bytes::new())).await,
    );
    // then: the replica is unchanged: K1 is as before, K2 did not appear (not even as a tombstone)
    let all = scan(&r, b"").await;
    assert_eq!(all.len(), 1, "replica after the refusal: {all:?}");
    assert_eq!(all[0].value, val("v1"));
}

pub(crate) async fn oversize_value_rejected(pair: SyncPair) {
    // given: an empty replica of A
    let r = open_net(&pair.a).await;
    // when: A writes a value one byte over the limit
    let too_big = Bytes::from(vec![b'x'; MAX_VALUE_BYTES + 1]);
    expect_config("set_own(MAX+1)", must_finish(r.set_own(K1, too_big)).await);
    // then: Config and the replica is empty
    assert!(scan(&r, b"").await.is_empty(), "the refusal left an entry");
    // and exactly the limit is accepted
    let edge = Bytes::from(vec![b'x'; MAX_VALUE_BYTES]);
    expect_ok("set_own(MAX)", must_finish(r.set_own(K1, edge)).await);
    let got = get(&r, r.me(), K1).await.expect("no entry");
    assert_eq!(got.value.len(), MAX_VALUE_BYTES);
}

pub(crate) async fn del_own_hides_and_leaves_tombstone(pair: SyncPair) {
    // given: A wrote K1
    let r = open_net(&pair.a).await;
    put(&r, K1, "v1").await;
    // when: A deletes K1
    expect_ok("del_own", must_finish(r.del_own(K1)).await);
    // then: get does not see the entry, scan shows a tombstone, live filters it out
    assert!(
        get(&r, r.me(), K1).await.is_none(),
        "get sees the deleted entry"
    );
    let all = scan(&r, b"").await;
    assert_eq!(all.len(), 1, "expected one tombstone: {all:?}");
    assert_eq!((all[0].author, all[0].key.as_slice()), (r.me(), K1));
    assert!(all[0].is_tombstone(), "tombstone value is not empty");
    assert!(live(all).is_empty(), "live kept the tombstone");
}

pub(crate) async fn del_missing_key_is_ok(pair: SyncPair) {
    // given: an empty replica of A
    let r = open_net(&pair.a).await;
    // when: A deletes a key that never existed
    let res = must_finish(r.del_own(K1)).await;
    // then: Ok, there is no entry, but scan holds a tombstone
    expect_ok("del_own on a missing key", res);
    assert!(get(&r, r.me(), K1).await.is_none());
    let all = scan(&r, b"").await;
    assert!(
        all.len() == 1 && all[0].is_tombstone(),
        "expected a tombstone: {all:?}"
    );
}

pub(crate) async fn tombstone_emits_local_change(pair: SyncPair) {
    // given: A wrote K1, the subscription was created before the write
    let r = open_net(&pair.a).await;
    let mut events = r.subscribe();
    put(&r, K1, "v1").await;
    next_event(&mut events, "LocalChange from the write", is_local).await;
    // when: A deletes K1
    expect_ok("del_own", must_finish(r.del_own(K1)).await);
    // then: the tombstone also yields LocalChange
    next_event(&mut events, "LocalChange from the tombstone", is_local).await;
}

pub(crate) async fn ts_within_minute(pair: SyncPair) {
    // given: replica of A
    let r = open_net(&pair.a).await;
    // when: A writes an entry
    put(&r, K1, "v1").await;
    // then: ts_micros by the writer's clock is within a minute of "now"
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before 1970");
    let got = get(&r, r.me(), K1).await.expect("no entry");
    let skew = got.ts_micros.abs_diff(now.as_micros() as u64);
    assert!(
        skew <= 60_000_000,
        "ts_micros {} is far from now, difference {skew} us",
        got.ts_micros
    );
}

pub(crate) async fn local_change_on_write_not_on_read(pair: SyncPair) {
    // given: replica of A, subscription created
    let r = open_net(&pair.a).await;
    let mut events = r.subscribe();
    // when: A writes an entry
    put(&r, K1, "v1").await;
    // then: LocalChange arrived
    next_event(&mut events, "LocalChange from the write", is_local).await;
    // when: A only reads
    get(&r, r.me(), K1).await;
    scan(&r, b"").await;
    latest_per_key(&r, b"").await;
    // then: reading yields no changes
    assert_no_change(&mut events).await;
}

pub(crate) async fn domains_refuses_sync(pair: SyncPair) {
    // given: replica "domains" of node A
    let d = open(&pair.a, REPLICA_DOMAINS).await;
    // when: we ask to enable reconciliation and to reconcile with B
    let started = must_finish(d.start_sync()).await;
    let synced = must_finish(d.sync_with(pid(SEED_B))).await;
    // then: both calls return Refused
    assert!(
        matches!(started, Err(NetError::Refused)),
        "start_sync: {started:?}"
    );
    assert!(
        matches!(synced, Err(NetError::Refused)),
        "sync_with: {synced:?}"
    );
}

pub(crate) async fn unknown_replica_name_rejected(pair: SyncPair) {
    // given: sync of node A; when: we open a replica with an unknown name
    let res = must_finish(pair.a.open_replica("nope")).await;
    // then: Config
    expect_config("open_replica(nope)", res);
}

pub(crate) async fn same_name_same_replica(pair: SyncPair) {
    // given: replica "net" opened twice
    let first = open_net(&pair.a).await;
    let second = open_net(&pair.a).await;
    // when: an entry is written through the first handle
    put(&first, K1, "v1").await;
    // then: the second sees it, the ids are equal
    let got = get(&second, second.me(), K1)
        .await
        .expect("the second handle does not see the entry");
    assert_eq!(got.value, val("v1"));
    assert_eq!(first.id(), second.id(), "one name, different replicas");
}

#[path = "replica_cases_help.rs"]
mod help;
pub(crate) use help::*;
