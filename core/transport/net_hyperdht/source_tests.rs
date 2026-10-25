//! Tests of `HyperDhtDiscovery` on `MemRecords`: the shared discovery contract suite and source properties —
//! each key is handed out once, our own is skipped, store errors are survived, the `publish` record
//! is verified by reading. Pacing in milliseconds.

use super::*;
use crate::members::network_key::NetworkKey;
use crate::transport::net::contract::discovery::{addr_of, discovery_contract, feed};
use crate::transport::net::contract::{must_finish, pid, DiscoveryHarness, SEED_A, SEED_B, SEED_C};
use crate::transport::net::{BoxFuture, Discovery, NetResult, PeerAddr};
use crate::transport::net_hyperdht::mem::MemRecords;
use crate::transport::net_hyperdht::store::{Record, RecordKey, RecordStore};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

discovery_contract!(hyperdht_harness);

const MS: Duration = Duration::from_millis(1);

fn test_pace() -> Pace {
    Pace {
        fast_rounds: 1_000_000,
        fast: 30 * MS,
        steady: (30 * MS, 40 * MS),
        error_first: 10 * MS,
        error_max: 40 * MS,
        call_limit: 300 * MS,
        summary_every: Duration::from_secs(600),
    }
}

fn keys() -> RosterKeys {
    RosterKeys::from_subkeys(&NetworkKey::from_bytes([0x55; 32]).subkeys())
}

/// Discovery suite environment: the sources share one store; the `me` seed grows from 100.
struct HyperDhtHarness {
    store: Arc<MemRecords>,
    next: AtomicU8,
}

impl DiscoveryHarness for HyperDhtHarness {
    fn source(&self, publishes: bool) -> Arc<dyn Discovery> {
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let me = publishes.then(|| pid(n));
        Arc::new(HyperDhtDiscovery::new(self.store.clone(), keys(), me).with_pace(test_pace()))
    }
}

fn hyperdht_harness() -> HyperDhtHarness {
    HyperDhtHarness {
        store: Arc::new(MemRecords::new()),
        next: AtomicU8::new(100),
    }
}

fn source_over(store: Arc<dyn RecordStore>, me: Option<u8>) -> Arc<HyperDhtDiscovery> {
    Arc::new(HyperDhtDiscovery::new(store, keys(), me.map(pid)).with_pace(test_pace()))
}

/// A store that silently loses the first `drop` records (like a lost race on `seq`).
struct Lossy {
    inner: MemRecords,
    drop: AtomicUsize,
    puts: AtomicUsize,
}

impl Lossy {
    fn new(drop: usize) -> Arc<Self> {
        Arc::new(Self {
            inner: MemRecords::new(),
            drop: AtomicUsize::new(drop),
            puts: AtomicUsize::new(0),
        })
    }
}

impl RecordStore for Lossy {
    fn get(&self, key: &RecordKey) -> BoxFuture<'_, NetResult<Option<Record>>> {
        self.inner.get(key)
    }

    fn put(&self, key: &RecordKey, value: Vec<u8>, seq: u64) -> BoxFuture<'_, NetResult<()>> {
        self.puts.fetch_add(1, Ordering::SeqCst);
        // `try_update` needs Rust 1.95, above the MSRV (1.94).
        #[allow(deprecated)]
        let lost = self
            .drop
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1));
        match lost {
            Ok(_) => Box::pin(async { Ok(()) }),
            Err(_) => self.inner.put(key, value, seq),
        }
    }
}

async fn stored(
    store: &dyn RecordStore,
) -> Vec<crate::transport::net_mainline::record::RosterEntry> {
    let (r, _) = roster::read(store, &keys())
        .await
        .expect("read")
        .expect("record exists");
    r.entries
}

async fn collect(
    rx: &mut tokio::sync::mpsc::Receiver<PeerAddr>,
    window: Duration,
) -> Vec<PeerAddr> {
    let mut got = Vec::new();
    let _ = tokio::time::timeout(window, async {
        while let Some(a) = rx.recv().await {
            got.push(a);
        }
    })
    .await;
    got
}

#[tokio::test]
async fn run_reports_each_key_once() {
    let store = Arc::new(MemRecords::new());
    let (a, b) = (
        source_over(store.clone(), Some(SEED_A)),
        source_over(store.clone(), Some(SEED_B)),
    );
    must_finish(a.publish(&addr_of(SEED_A))).await.expect("A");
    must_finish(b.publish(&addr_of(SEED_B))).await.expect("B");
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let task = crate::node::output::spawn(source_over(store, Some(SEED_C)).run_source(tx));
    let got = collect(&mut rx, Duration::from_millis(400)).await;
    task.abort();
    let mut ids: Vec<_> = got.iter().map(|a| a.id).collect();
    ids.sort_by_key(|i| *i.as_bytes());
    let mut want = vec![pid(SEED_A), pid(SEED_B)];
    want.sort_by_key(|i| *i.as_bytes());
    assert_eq!(ids, want, "each key once over several rounds");
}

#[tokio::test]
async fn run_skips_own_key() {
    let store = Arc::new(MemRecords::new());
    let a = source_over(store.clone(), Some(SEED_A));
    let b = source_over(store, Some(SEED_B));
    must_finish(b.publish(&addr_of(SEED_B))).await.expect("B");
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let task = crate::node::output::spawn(a.run_source(tx));
    let got = collect(&mut rx, Duration::from_millis(300)).await;
    task.abort();
    assert!(
        got.iter().all(|x| x.id != pid(SEED_A)),
        "own key handed out: {got:?}"
    );
    assert!(got.iter().any(|x| x.id == pid(SEED_B)));
}

#[tokio::test]
async fn run_backs_off_on_errors_then_recovers() {
    let store = Arc::new(MemRecords::new());
    let a = source_over(store.clone(), Some(SEED_A));
    must_finish(a.publish(&addr_of(SEED_A))).await.expect("A");
    store.fail_next(3);
    let reader = source_over(store, None);
    let mut found = feed(&(reader as Arc<dyn Discovery>));
    assert!(
        found.sees(pid(SEED_A)).await,
        "key not found after three errors"
    );
}

#[tokio::test]
async fn stalled_store_hits_call_limit() {
    let store = Arc::new(MemRecords::new());
    store.stall(true);
    let a = source_over(store, Some(SEED_A));
    let got = tokio::time::timeout(Duration::from_secs(3), a.publish(&addr_of(SEED_A))).await;
    assert!(got.expect("publish hung beyond the deadline").is_err());
}

#[tokio::test]
async fn publish_retries_when_record_lost() {
    let store = Lossy::new(1);
    let a = source_over(store.clone(), Some(SEED_A));
    must_finish(a.publish(&addr_of(SEED_A)))
        .await
        .expect("publish");
    assert_eq!(store.puts.load(Ordering::SeqCst), 2);
    assert!(stored(&*store)
        .await
        .iter()
        .any(|e| e.id == *pid(SEED_A).as_bytes()));
}

#[tokio::test]
async fn publish_gives_up_after_three_tries() {
    let store = Lossy::new(usize::MAX);
    let a = source_over(store.clone(), Some(SEED_A));
    let got = must_finish(a.publish(&addr_of(SEED_A))).await;
    assert!(got.is_err());
    assert_eq!(store.puts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn publish_ignores_hints() {
    let store = Arc::new(MemRecords::new());
    let a = source_over(store.clone(), Some(SEED_A));
    must_finish(a.publish(&addr_of(SEED_A)))
        .await
        .expect("publish");
    let entries = stored(&*store).await;
    assert_eq!(store.puts(), 1, "one write per publish");
    assert!(entries
        .iter()
        .all(|e| e.relay.is_none() && e.addrs.is_empty()));
}

#[test]
fn node_source_publishes() {
    let sub = NetworkKey::from_bytes([0x55; 32]).subkeys();
    let s = node_source(&sub, pid(SEED_A), Arc::new(MemRecords::new()), None);
    assert!(s.publishes());
    assert_eq!(s.name(), "hyperdht");
}

fn recorder() -> (RoundOutcomeFn, Arc<std::sync::Mutex<Vec<DhtRoundOutcome>>>) {
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = log.clone();
    (Arc::new(move |s| sink.lock().unwrap().push(s)), log)
}

#[tokio::test]
async fn reader_reports_read_with_member_count() {
    let store = Arc::new(MemRecords::new());
    must_finish(source_over(store.clone(), Some(SEED_A)).publish(&addr_of(SEED_A)))
        .await
        .expect("A");
    must_finish(source_over(store.clone(), Some(SEED_B)).publish(&addr_of(SEED_B)))
        .await
        .expect("B");
    let (f, log) = recorder();
    let reader = Arc::new(
        HyperDhtDiscovery::new(store, keys(), None)
            .with_pace(test_pace())
            .with_round_outcome(f),
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let task = crate::node::output::spawn(reader.run_source(tx));
    let _ = collect(&mut rx, Duration::from_millis(150)).await;
    task.abort();
    let first = log.lock().unwrap()[0];
    assert_eq!(
        first,
        DhtRoundOutcome {
            read: true,
            members: 2,
            published: false
        }
    );
}

#[tokio::test]
async fn get_error_reports_not_read() {
    let store = Arc::new(MemRecords::new());
    store.fail_next(2);
    let (f, log) = recorder();
    let reader = Arc::new(
        HyperDhtDiscovery::new(store, keys(), None)
            .with_pace(test_pace())
            .with_round_outcome(f),
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let task = crate::node::output::spawn(reader.run_source(tx));
    let _ = collect(&mut rx, Duration::from_millis(15)).await;
    task.abort();
    let seen = log.lock().unwrap().clone();
    assert!(!seen.is_empty());
    assert_eq!(
        seen[0],
        DhtRoundOutcome {
            read: false,
            members: 0,
            published: false
        }
    );
}

#[tokio::test]
async fn publisher_reports_published() {
    let (f, log) = recorder();
    let a = Arc::new(
        HyperDhtDiscovery::new(Arc::new(MemRecords::new()), keys(), Some(pid(SEED_A)))
            .with_pace(test_pace())
            .with_round_outcome(f),
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let task = crate::node::output::spawn(a.run_source(tx));
    let _ = collect(&mut rx, Duration::from_millis(100)).await;
    task.abort();
    assert_eq!(
        log.lock().unwrap()[0],
        DhtRoundOutcome {
            read: true,
            members: 1,
            published: true
        }
    );
}

mod account {
    use super::*;
    use crate::transport::net_mainline::record::RosterEntry;

    mod successful_round {
        use super::*;

        #[test]
        fn returns_the_other_keys_without_mine_in_record_order() {
            let mut f = setup();
            let got = f.when_account_ok(vec![SEED_A, SEED_C, SEED_B]);
            assert_eq!(got, Some(vec![pid(SEED_A), pid(SEED_B)]));
        }

        #[test]
        fn resets_the_failure_streak() {
            let mut f = setup();
            f.failures = 2;
            f.when_account_ok(vec![SEED_A, SEED_C]);
            assert_eq!(f.failures, 0, "a successful round clears the streak");
        }

        #[test]
        fn tally_counts_one_round_and_no_error() {
            let mut f = setup();
            f.when_account_ok(vec![SEED_A, SEED_C]);
            assert_eq!(f.tally.counts(), (1, 0));
        }

        #[test]
        fn reports_read_members_and_published() {
            let mut f = setup();
            f.when_account_ok(vec![SEED_A, SEED_B, SEED_C]);
            assert_eq!(
                f.outcomes(),
                vec![DhtRoundOutcome {
                    read: true,
                    members: 3,
                    published: true
                }]
            );
        }
    }

    mod failed_round {
        use super::*;

        #[test]
        fn returns_nothing() {
            let mut f = setup();
            assert_eq!(f.when_account_timeout(), None);
        }

        #[test]
        fn counts_one_more_failure() {
            let mut f = setup();
            f.when_account_timeout();
            assert_eq!(f.failures, 1);
        }

        #[test]
        fn failures_accumulate_across_rounds() {
            let mut f = setup();
            f.failures = 1;
            f.when_account_timeout();
            assert_eq!(f.failures, 2);
        }

        #[test]
        fn tally_counts_one_round_and_one_error() {
            let mut f = setup();
            f.when_account_timeout();
            assert_eq!(f.tally.counts(), (1, 1));
        }

        #[test]
        fn reports_not_read_with_no_members() {
            let mut f = setup();
            f.when_account_timeout();
            assert_eq!(
                f.outcomes(),
                vec![DhtRoundOutcome {
                    read: false,
                    members: 0,
                    published: false
                }]
            );
        }
    }

    struct Fixture {
        source: HyperDhtDiscovery,
        tally: Tally,
        failures: u32,
        seen: Arc<std::sync::Mutex<Vec<DhtRoundOutcome>>>,
    }

    impl Fixture {
        fn when_account_ok(&mut self, ids: Vec<u8>) -> Option<Vec<PeerId>> {
            let entries = ids.into_iter().map(entry).collect();
            let roster = Roster {
                version: crate::transport::net_mainline::record::RECORD_VERSION_KEYS,
                entries,
            };
            self.source
                .account(Ok(roster), &mut self.tally, &mut self.failures)
        }

        fn when_account_timeout(&mut self) -> Option<Vec<PeerId>> {
            let res = Err(NetError::Timeout);
            self.source
                .account(res, &mut self.tally, &mut self.failures)
        }

        fn outcomes(&self) -> Vec<DhtRoundOutcome> {
            self.seen.lock().unwrap().clone()
        }
    }

    /// A publishing source whose own key is `SEED_C`, with a fresh tally and a zero failure streak.
    fn setup() -> Fixture {
        let (f, seen) = recorder();
        let source = HyperDhtDiscovery::new(Arc::new(MemRecords::new()), keys(), Some(pid(SEED_C)))
            .with_pace(test_pace())
            .with_round_outcome(f);
        let tally = Tally::new(Instant::now(), Duration::from_secs(600));
        Fixture {
            source,
            tally,
            failures: 0,
            seen,
        }
    }

    fn entry(seed: u8) -> RosterEntry {
        RosterEntry {
            id: *pid(seed).as_bytes(),
            relay: None,
            addrs: Vec::new(),
            seen_unix: 1,
        }
    }
}
