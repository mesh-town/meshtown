//! Record store on a loopback cluster: a "write — read" round trip through two different nodes.

use crate::net_hyper::test_cluster::{cluster, Cluster};
use crate::net_hyper::{HyperOpts, HyperTransport};
use crate::net::contract::secret;
use crate::net::addr::PeerAddr;
use crate::net::{Discovery, Transport};
use crate::net_hyperdht::{HyperDhtDiscovery, Pace, Record, RecordKey, RecordStore, RosterKeys};
use crate::network_key::NetworkKey;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

const WITHIN: Duration = Duration::from_secs(30);

struct Pair {
    _cluster: Cluster,
    a: Arc<dyn RecordStore>,
    b: Arc<dyn RecordStore>,
}

async fn pair() -> Pair {
    let cluster = cluster(3).await;
    let opts = || HyperOpts { bootstrap: Some(cluster.bootstrap()), ..Default::default() };
    let a = HyperTransport::bind(secret(1), opts()).await.expect("node A");
    let b = HyperTransport::bind(secret(2), opts()).await.expect("node B");
    Pair { _cluster: cluster, a: a.records(), b: b.records() }
}

fn key(n: u8) -> RecordKey {
    RecordKey::from_seed([n; 32])
}

#[tokio::test]
async fn put_then_get_round_trip() {
    let p = pair().await;
    timeout(WITHIN, p.a.put(&key(7), b"v".to_vec(), 5)).await.expect("put hung").expect("put");
    let got = timeout(WITHIN, p.b.get(&key(7))).await.expect("get hung").expect("get");
    assert_eq!(got, Some(Record { seq: 5, value: b"v".to_vec() }));
}

#[tokio::test]
async fn get_missing_is_none() {
    let p = pair().await;
    let got = timeout(WITHIN, p.b.get(&key(8))).await.expect("get hung").expect("get");
    assert_eq!(got, None);
}

#[tokio::test]
async fn lower_seq_does_not_replace() {
    let p = pair().await;
    timeout(WITHIN, p.a.put(&key(9), b"new".to_vec(), 9)).await.expect("put 9 hung").expect("put 9");
    let _ = timeout(WITHIN, p.a.put(&key(9), b"old".to_vec(), 3)).await.expect("put 3 hung");
    let got = timeout(WITHIN, p.b.get(&key(9))).await.expect("get hung").expect("get");
    assert_eq!(got, Some(Record { seq: 9, value: b"new".to_vec() }));
}

const MS: Duration = Duration::from_millis(1);

fn quick_pace() -> Pace {
    Pace {
        fast_rounds: 1_000_000,
        fast: 300 * MS,
        steady: (300 * MS, 400 * MS),
        error_first: 100 * MS,
        error_max: 400 * MS,
        call_limit: 10 * WITHIN,
        summary_every: Duration::from_secs(600),
    }
}

async fn seen(out: &mut mpsc::Receiver<PeerAddr>, want: &PeerAddr) {
    while let Some(addr) = out.recv().await {
        if addr.id == want.id {
            return;
        }
    }
    panic!("source channel closed, the key did not arrive");
}

#[tokio::test]
async fn two_nodes_find_each_other_through_record() {
    let cluster = cluster(3).await;
    let opts = || HyperOpts { bootstrap: Some(cluster.bootstrap()), ..Default::default() };
    let a = HyperTransport::bind(secret(1), opts()).await.expect("node A");
    let b = HyperTransport::bind(secret(2), opts()).await.expect("node B");
    let keys = RosterKeys::from_subkeys(&NetworkKey::from_bytes([0x55; 32]).subkeys());
    let source = |t: &Arc<HyperTransport>| {
        Arc::new(HyperDhtDiscovery::new(t.records(), keys.clone(), Some(t.me())).with_pace(quick_pace()))
    };
    let (src_a, src_b) = (source(&a), source(&b));
    let (tx_a, mut out_a) = mpsc::channel(16);
    let (tx_b, mut out_b) = mpsc::channel(16);
    let run_a = tokio::spawn(src_a.run_source(tx_a));
    let run_b = tokio::spawn(src_b.run_source(tx_b));
    timeout(WITHIN, seen(&mut out_a, &PeerAddr::new(b.me()))).await.expect("A did not see B");
    timeout(WITHIN, seen(&mut out_b, &PeerAddr::new(a.me()))).await.expect("B did not see A");
    run_a.abort();
    run_b.abort();
}
