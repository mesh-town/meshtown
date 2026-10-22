//! Cases of the replica contract on two nodes (`replica_contract!`): reconciliation, `latest_per_key`, the closed replica "domains".
//! Data on the other side is awaited by polling up to `STEP` (`wait_scan`); `Synced` is used only as an event for the log
//! and as "reconciliation finished" where the data has already converged. Entries of different authors are separated by `PAUSE`.

use crate::transport::net::contract::replica_cases::{
    expect_ok, get, is_remote, latest_per_key, next_event, open, open_net, put, scan, val,
    wait_scan, Rep, K1, K2, K3, PAUSE, QUIET,
};
use crate::transport::net::contract::{must_finish, pid, SyncPair, SEED_A, SEED_B};
use crate::transport::net::id::PeerId;
use crate::transport::net::replica::{live, ReplicaEntry, ReplicaEvent};
use crate::transport::net::sync_backend::REPLICA_DOMAINS;

/// Time of the entries of the "tie" case (Unix microseconds): one for both entries.
const TIE_TS: u64 = 1_700_000_000_000_000;

pub(crate) async fn scan_order_author_then_key(pair: SyncPair) {
    // given: A writes K3 and K1, B writes K2 and K1; reconciliation is on
    let (ra, rb) = started_pair(&pair).await;
    put(&ra, K3, "a3").await;
    put(&ra, K1, "a1").await;
    put(&rb, K2, "b2").await;
    put(&rb, K1, "b1").await;
    // when: A has reconciled with B and sees all four entries
    sync_from_a(&ra).await;
    let all = wait_scan(&ra, "four entries at A", |es| es.len() == 4).await;
    // then: the order is (author bytes, key) ascending
    let mut want = vec![
        (pid(SEED_A), K1),
        (pid(SEED_A), K3),
        (pid(SEED_B), K1),
        (pid(SEED_B), K2),
    ];
    want.sort_by(|x, y| (x.0.as_bytes(), x.1).cmp(&(y.0.as_bytes(), y.1)));
    let got: Vec<(PeerId, &[u8])> = all.iter().map(|e| (e.author, e.key.as_slice())).collect();
    assert_eq!(got, want, "scan order is not (author, key)");
    // and a prefix selects the keys of all authors
    let by_prefix = scan(&ra, K1).await;
    assert_eq!(by_prefix.len(), 2, "prefix K1: {by_prefix:?}");
    assert!(by_prefix.iter().all(|e| e.key == K1));
}

pub(crate) async fn sync_delivers_and_reports_synced(pair: SyncPair) {
    // given: A has entry K1, B has entry K2; reconciliation is on; A is subscribed before sync_with
    let (ra, rb) = started_pair(&pair).await;
    put(&ra, K1, "a1").await;
    put(&rb, K2, "b2").await;
    let mut events = ra.subscribe();
    // when: A reconciles with B
    sync_from_a(&ra).await;
    // then: the result is a Synced event with peer = B. Its outcome is "for the log only": iroh-docs 0.101.0
    // on a loopback ends reconciliation with `Err("Failed to close connection1")` even when the data has converged,
    // so success is not required — the data is checked below by polling
    let synced = next_event(&mut events, "Synced from B", is_synced_with_b).await;
    assert!(
        matches!(synced, ReplicaEvent::Synced { .. }),
        "reconciliation result: {synced:?}"
    );
    // and the entries arrive in both directions (by polling, not by Synced)
    wait_scan(&ra, "entry of B at A", |es| has(es, pid(SEED_B), K2, "b2")).await;
    wait_scan(&rb, "entry of A at B", |es| has(es, pid(SEED_A), K1, "a1")).await;
}

pub(crate) async fn repeat_sync_no_duplicates(pair: SyncPair) {
    // given: A and B have converged: both have two entries
    let (ra, rb) = started_pair(&pair).await;
    put(&ra, K1, "a1").await;
    put(&rb, K2, "b2").await;
    sync_from_a(&ra).await;
    wait_scan(&ra, "two entries at A", |es| es.len() == 2).await;
    wait_scan(&rb, "two entries at B", |es| es.len() == 2).await;
    let (before_a, before_b) = (scan(&ra, b"").await, scan(&rb, b"").await);
    // when: A reconciles with B two more times, each time waiting for the result
    for _ in 0..2 {
        let mut events = ra.subscribe();
        sync_from_a(&ra).await;
        next_event(
            &mut events,
            "result of the repeated reconciliation",
            is_synced_with_b,
        )
        .await;
    }
    tokio::time::sleep(QUIET / 3).await;
    // then: the same number of entries and they are unchanged — on both sides
    assert_eq!(scan(&ra, b"").await, before_a, "at A after the repeat");
    assert_eq!(scan(&rb, b"").await, before_b, "at B after the repeat");
}

pub(crate) async fn live_push_after_sync_emits_remote_change(pair: SyncPair) {
    // given: empty A and B have already reconciled, B is subscribed
    let (ra, rb) = started_pair(&pair).await;
    let mut events_a = ra.subscribe();
    sync_from_a(&ra).await;
    next_event(
        &mut events_a,
        "result of the first reconciliation",
        is_synced_with_b,
    )
    .await;
    let mut events_b = rb.subscribe();
    // when: A writes a new entry, there is no second reconciliation
    put(&ra, K1, "a1").await;
    // then: B gets RemoteChange and sees the entry of A
    next_event(&mut events_b, "RemoteChange at B", is_remote).await;
    wait_scan(&rb, "entry of A at B", |es| has(es, pid(SEED_A), K1, "a1")).await;
}

pub(crate) async fn tombstone_travels(pair: SyncPair) {
    // given: B wrote K1 earlier, A later; both entries reached B
    let (ra, rb) = started_pair(&pair).await;
    put(&rb, K1, "b1").await;
    tokio::time::sleep(PAUSE).await;
    put(&ra, K1, "a1").await;
    sync_from_a(&ra).await;
    wait_scan(&rb, "entry of A at B", |es| has(es, pid(SEED_A), K1, "a1")).await;
    tokio::time::sleep(PAUSE).await;
    let mut events_b = rb.subscribe();
    // when: A deletes K1 and reconciles once more
    expect_ok("del_own", must_finish(ra.del_own(K1)).await);
    sync_from_a(&ra).await;
    // then: at B the entry of A became a tombstone and RemoteChange arrived
    let all = wait_scan(&rb, "tombstone of A at B", |es| {
        tombstone_of(es, pid(SEED_A), K1)
    })
    .await;
    assert!(
        get(&rb, pid(SEED_A), K1).await.is_none(),
        "get sees the deleted entry: {all:?}"
    );
    next_event(&mut events_b, "RemoteChange from the tombstone", is_remote).await;
    // and in latest_per_key it overrides the older entry of B with the same key
    let newest = latest_per_key(&rb, K1).await;
    assert!(
        newest.len() == 1 && newest[0].is_tombstone(),
        "latest: {newest:?}"
    );
    assert_eq!(newest[0].author, pid(SEED_A));
    assert!(live(newest).is_empty(), "live kept the tombstone");
}

pub(crate) async fn latest_picks_newest_across_authors(pair: SyncPair) {
    // given: A writes K1 and K2; later B writes K1; everything has converged
    let (ra, rb) = started_pair(&pair).await;
    put(&ra, K1, "old").await;
    put(&ra, K2, "only-a").await;
    tokio::time::sleep(PAUSE).await;
    put(&rb, K1, "new").await;
    sync_from_a(&ra).await;
    wait_scan(&ra, "three entries at A", |es| es.len() == 3).await;
    wait_scan(&rb, "three entries at B", |es| es.len() == 3).await;
    // when: both sides ask for latest_per_key
    // then: one entry per key — the newest; ordered by key; the sides agree
    for (name, r) in [("A", &ra), ("B", &rb)] {
        let got = latest_per_key(r, b"").await;
        let keys: Vec<&[u8]> = got.iter().map(|e| e.key.as_slice()).collect();
        assert_eq!(keys, vec![K1, K2], "latest keys at {name}");
        assert_eq!(
            (got[0].author, got[0].value.clone()),
            (pid(SEED_B), val("new")),
            "K1 at {name}"
        );
        assert_eq!(
            (got[1].author, got[1].value.clone()),
            (pid(SEED_A), val("only-a")),
            "K2 at {name}"
        );
    }
    assert_eq!(latest_per_key(&ra, K2).await.len(), 1, "prefix K2");
}

pub(crate) async fn latest_tie_breaks_by_author(pair: SyncPair) {
    // iroh sets the time itself: a tie can be built only where a "raw" entry exists
    let Some(raw) = pair.raw.clone() else { return };
    // given: keys K1 and K2 each have two entries with the same ts_micros from authors A and B, in different order
    let ra = open_net(&pair.a).await;
    let (low, high) = authors_by_bytes();
    let tied = |author: PeerId, key: &[u8], value: &str| ReplicaEntry {
        author,
        key: key.to_vec(),
        value: val(value),
        ts_micros: TIE_TS,
    };
    raw(tied(low, K1, "low"));
    raw(tied(high, K1, "high"));
    raw(tied(high, K2, "high"));
    raw(tied(low, K2, "low"));
    // when: we ask for latest_per_key
    let got = latest_per_key(&ra, b"").await;
    // then: for each key the author with the larger PeerId bytes wins, the order of entries does not matter
    assert_eq!(got.len(), 2, "latest: {got:?}");
    for e in &got {
        assert_eq!(
            (e.author, e.value.clone()),
            (high, val("high")),
            "key {:?}",
            e.key
        );
    }
}

pub(crate) async fn same_key_two_authors_two_entries(pair: SyncPair) {
    // given: A and B wrote one key K1 with their own values; converged
    let (ra, rb) = started_pair(&pair).await;
    put(&ra, K1, "va").await;
    put(&rb, K1, "vb").await;
    sync_from_a(&ra).await;
    wait_scan(&ra, "two entries at A", |es| es.len() == 2).await;
    // then: two entries per key, each author has its own
    let a_entry = get(&ra, pid(SEED_A), K1).await.expect("no entry of A");
    let b_entry = get(&ra, pid(SEED_B), K1).await.expect("no entry of B");
    assert_eq!((a_entry.value, b_entry.value), (val("va"), val("vb")));
    // when: A overwrites its own key
    put(&ra, K1, "va2").await;
    // then: the entry of B is untouched, there are still two entries
    let kept = get(&ra, pid(SEED_B), K1).await.expect("entry of B is gone");
    assert_eq!(kept.value, val("vb"));
    assert_eq!(scan(&ra, K1).await.len(), 2);
}

pub(crate) async fn domains_stays_private(pair: SyncPair) {
    // given: A has entry K1 in "domains" and marker K2 in "net"; "net" reconciliation is on on both sides
    let (ra, rb) = started_pair(&pair).await;
    let da = open(&pair.a, REPLICA_DOMAINS).await;
    let db = open(&pair.b, REPLICA_DOMAINS).await;
    put(&da, K1, "secret").await;
    put(&ra, K2, "marker").await;
    // when: "net" has been reconciled — the marker reached B
    sync_from_a(&ra).await;
    wait_scan(&rb, "marker of A at B", |es| {
        has(es, pid(SEED_A), K2, "marker")
    })
    .await;
    tokio::time::sleep(QUIET / 3).await;
    // then: "domains" of B is empty, "net" of B has one marker entry, "net" of A has no entry from "domains"
    assert!(
        scan(&db, b"").await.is_empty(),
        "the \"domains\" replica of B sees foreign data"
    );
    assert!(
        get(&db, pid(SEED_A), K1).await.is_none(),
        "B's \"domains\" replica sees A's entry"
    );
    assert_eq!(scan(&rb, b"").await.len(), 1, "extra entry in net of B");
    assert!(
        scan(&ra, K1).await.is_empty(),
        "the domains entry leaked into net of A"
    );
}

/// Replicas "net" of both nodes with reconciliation on.
async fn started_pair(pair: &SyncPair) -> (Rep, Rep) {
    let (ra, rb) = (open_net(&pair.a).await, open_net(&pair.b).await);
    expect_ok("start_sync A", must_finish(ra.start_sync()).await);
    expect_ok("start_sync B", must_finish(rb.start_sync()).await);
    (ra, rb)
}

/// A reconciles with B: `Ok` means "started", the result arrives as an event.
async fn sync_from_a(ra: &Rep) {
    expect_ok("sync_with(B)", must_finish(ra.sync_with(pid(SEED_B))).await);
}

fn is_synced_with_b(e: &ReplicaEvent) -> bool {
    matches!(e, ReplicaEvent::Synced { peer, .. } if *peer == pid(SEED_B))
}

fn has(entries: &[ReplicaEntry], author: PeerId, key: &[u8], value: &str) -> bool {
    let want = val(value);
    entries
        .iter()
        .any(|e| e.author == author && e.key == key && e.value == want)
}

fn tombstone_of(entries: &[ReplicaEntry], author: PeerId, key: &[u8]) -> bool {
    entries
        .iter()
        .any(|e| e.author == author && e.key == key && e.is_tombstone())
}

/// Authors A and B: first with smaller PeerId bytes, then with larger ones.
fn authors_by_bytes() -> (PeerId, PeerId) {
    let (a, b) = (pid(SEED_A), pid(SEED_B));
    assert_ne!(a.as_bytes(), b.as_bytes(), "the seeds gave one PeerId");
    if a.as_bytes() < b.as_bytes() {
        (a, b)
    } else {
        (b, a)
    }
}
