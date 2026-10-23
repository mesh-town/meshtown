//! Probes on a loopback cluster, without adapter code: the townjump crates directly.
//! The results and conclusions are recorded in the transport design notes.
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use townjump_dht::{ConnectOpts, Dht, DhtConnection, DhtFactory, DhtServer, Incoming, ServerOpts};
use townjump_dht_peeroxide::{PeeroxideDht, PeeroxideFactory, PeeroxideServer};
use townjump_keys::{key_pair_from_seed, KeyPair};

use super::test_cluster::{cluster, Cluster};

type Handle = PeeroxideServer;

fn key(n: u8) -> KeyPair {
    key_pair_from_seed(&[n; 32])
}

async fn node(cl: &Cluster, kp: &KeyPair) -> Arc<PeeroxideDht> {
    let factory = PeeroxideFactory {
        bootstrap: Some(cl.bootstrap()),
        ..Default::default()
    };
    Arc::new(factory.create(Some(kp.clone())).await.expect("node"))
}

/// A server on key `kp`: accepts connections and returns every byte packet it reads back.
async fn serve(dht: &PeeroxideDht, kp: &KeyPair) -> Handle {
    let (server, incoming) = dht.create_server(ServerOpts::default()).expect("server");
    server.listen(kp.clone()).await.expect("listen");
    crate::node::output::spawn(echo_loop(incoming));
    server
}

async fn echo_loop(mut incoming: Incoming) {
    while let Some(conn) = incoming.accept().await {
        crate::node::output::spawn(echo_one(conn));
    }
}

async fn echo_one(mut conn: DhtConnection) {
    let mut buf = [0u8; 16];
    while let Ok(n) = conn.stream.read(&mut buf).await {
        if n == 0 || conn.stream.write_all(&buf[..n]).await.is_err() {
            return;
        }
    }
}

/// Writes 8 bytes and waits for them back.
async fn ping(conn: &mut DhtConnection) {
    conn.stream.write_all(b"ping-001").await.expect("write");
    let mut back = [0u8; 8];
    conn.stream.read_exact(&mut back).await.expect("read");
    assert_eq!(&back, b"ping-001");
}

async fn dial(dht: &PeeroxideDht, to: &KeyPair) -> DhtConnection {
    let opts = ConnectOpts::default();
    dht.connect(to.public_key, opts).await.expect("connect")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_node_listens_and_dials() {
    let cl = cluster(3).await;
    let (ka, kb) = (key(1), key(2));
    let (a, b) = (node(&cl, &ka).await, node(&cl, &kb).await);
    let _sa = serve(&a, &ka).await;
    let _sb = serve(&b, &kb).await;
    let started = Instant::now();
    ping(&mut dial(&a, &kb).await).await;
    println!("PROBE1 A(server)->B: {:?}", started.elapsed());
    ping(&mut dial(&b, &ka).await).await;
    println!("PROBE1 B(server)->A: {:?}", started.elapsed());
    let me = tokio::time::timeout(
        Duration::from_secs(10),
        a.connect(ka.public_key, Default::default()),
    );
    println!(
        "PROBE1 dialing itself: {:?}",
        me.await.map(|r| r.map(|_| ()))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dial_unknown_key_fails_within_five_seconds() {
    let cl = cluster(3).await;
    let client = node(&cl, &key(3)).await;
    let started = Instant::now();
    let limit = Duration::from_secs(5);
    let attempt = client.connect(key(99).public_key, ConnectOpts::default());
    let got = tokio::time::timeout(limit, attempt).await;
    println!(
        "PROBE2 error: {:?} in {:?}",
        got.as_ref().map(|r| r.as_ref().err()),
        started.elapsed()
    );
    assert!(
        matches!(got, Ok(Err(_))),
        "expected an error faster than 5 s"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restarted_server_with_same_key_is_reachable() {
    let cl = cluster(3).await;
    let (ks, kc) = (key(4), key(5));
    let (first, client) = (node(&cl, &ks).await, node(&cl, &kc).await);
    let server = serve(&first, &ks).await;
    ping(&mut dial(&client, &ks).await).await;
    server.close().await.expect("close");
    first.destroy().await.expect("destroy");
    let started = Instant::now();
    let second = node(&cl, &ks).await;
    let _server = serve(&second, &ks).await;
    let mut attempts = 0;
    let mut conn = loop {
        attempts += 1;
        let step = Duration::from_secs(5);
        let try_dial = client.connect(ks.public_key, ConnectOpts::default());
        match tokio::time::timeout(step, try_dial).await {
            Ok(Ok(conn)) => break conn,
            other => println!("PROBE3 attempt {attempts}: {:?}", other.map(|r| r.err())),
        }
        assert!(attempts < 6, "server not found within 6 attempts");
    };
    ping(&mut conn).await;
    println!(
        "PROBE3 found in {:?}, attempts {attempts}",
        started.elapsed()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hundred_parallel_dials_finish() {
    let cl = cluster(3).await;
    let (ks, kc) = (key(6), key(7));
    let (srv, client) = (node(&cl, &ks).await, node(&cl, &kc).await);
    let _server = serve(&srv, &ks).await;
    let started = Instant::now();
    let tasks: Vec<_> = (0..100)
        .map(|_| crate::node::output::spawn(timed_dial(client.clone(), ks.clone())))
        .collect();
    let mut times = Vec::new();
    for task in tasks {
        times.push(task.await.expect("task"));
    }
    times.sort();
    let (p50, p95, max) = (times[49], times[94], times[99]);
    println!(
        "PROBE4 open p50={p50:?} p95={p95:?} max={max:?}, total {:?}",
        started.elapsed()
    );
    assert!(max < Duration::from_secs(10), "slowest open {max:?}");
}

async fn timed_dial(client: Arc<PeeroxideDht>, to: KeyPair) -> Duration {
    let started = Instant::now();
    let mut conn = dial(&client, &to).await;
    let opened = started.elapsed();
    ping(&mut conn).await;
    opened
}
