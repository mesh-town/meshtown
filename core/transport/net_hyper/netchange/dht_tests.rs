//! `DhtReact` on a real loopback cluster (real clock): closing sessions, `suspend`/`resume`, dial gate.

use super::DhtReact;
use crate::transport::net::contract::{secret, STEP};
use crate::transport::net::transport::{PeerConn, NET_CHANGED};
use crate::transport::net::{Transport, AUTH, PORT};
use crate::transport::net_hyper::netchange::react::{NetReact, CODE_NET_CHANGED};
use crate::transport::net_hyper::test_cluster::{cluster, Cluster};
use crate::transport::net_hyper::{HyperOpts, HyperTransport};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;

const CONNECT_LIMIT: Duration = Duration::from_secs(10);

async fn node(cl: &Cluster, seed: u8) -> Arc<HyperTransport> {
    let opts = HyperOpts {
        bootstrap: Some(cl.bootstrap()),
        ..Default::default()
    };
    let node = HyperTransport::bind(secret(seed), opts)
        .await
        .expect("node came up");
    node.serve(&[AUTH, PORT]);
    node.wait_listen_outcome().await.expect("listen succeeded");
    node
}

async fn dial(from: &HyperTransport, to: &HyperTransport) -> Arc<dyn PeerConn> {
    timeout(CONNECT_LIMIT, from.connect(&to.local_addr(), PORT))
        .await
        .expect("deadline")
        .expect("connection")
}

async fn pair() -> (Cluster, Arc<HyperTransport>, Arc<HyperTransport>) {
    let cl = cluster(3).await;
    let (a, b) = (node(&cl, 1).await, node(&cl, 2).await);
    (cl, a, b)
}

#[tokio::test]
async fn net_change_closes_live_session_with_reason() {
    let (_cl, a, b) = pair().await;
    let conn = dial(&a, &b).await;
    let (_svc, theirs) = timeout(STEP, b.accept())
        .await
        .expect("deadline")
        .expect("incoming");
    let react = a.net_react();
    react.close_sessions(CODE_NET_CHANGED, NET_CHANGED.as_bytes());
    react.suspend().await.expect("suspend");
    react.resume().await.expect("resume");
    timeout(Duration::from_secs(1), conn.closed())
        .await
        .expect("closed at A within 1 s");
    assert!(
        conn.close_reason().ends_with(NET_CHANGED),
        "reason: {}",
        conn.close_reason()
    );
    timeout(Duration::from_secs(25), theirs.closed())
        .await
        .expect("closed at B within 25 s");
}

#[tokio::test]
async fn connect_works_after_resume() {
    let (_cl, a, b) = pair().await;
    let _first = dial(&a, &b).await;
    let react = a.net_react();
    react.close_sessions(CODE_NET_CHANGED, NET_CHANGED.as_bytes());
    react.suspend().await.expect("suspend");
    react.resume().await.expect("resume");
    let again = dial(&a, &b).await;
    assert!(!again.is_closed());
}

#[tokio::test]
async fn close_sessions_counts_live_sessions() {
    let (_cl, a, b) = pair().await;
    let _one = dial(&a, &b).await;
    let _two = dial(&a, &b).await;
    assert_eq!(
        a.net_react()
            .close_sessions(CODE_NET_CHANGED, NET_CHANGED.as_bytes()),
        2
    );
}

#[tokio::test]
async fn connect_during_reaction_waits_for_resume() {
    let (_cl, a, b) = pair().await;
    let react: Arc<DhtReact> = Arc::new(a.net_react());
    react.close_sessions(CODE_NET_CHANGED, NET_CHANGED.as_bytes());
    react.suspend().await.expect("suspend");
    let resumed = Arc::new(std::sync::Mutex::new(None));
    let (r, mark) = (react.clone(), resumed.clone());
    crate::node::output::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        *mark.lock().unwrap() = Some(Instant::now());
        r.resume().await.expect("resume");
    });
    let conn = dial(&a, &b).await;
    let done = Instant::now();
    let at = resumed
        .lock()
        .unwrap()
        .expect("resume had already happened");
    assert!(done >= at, "connect finished before resume");
    assert!(!conn.is_closed());
}

#[test]
fn hyper_opts_default_does_not_watch() {
    assert!(!HyperOpts::default().watch_net);
}
