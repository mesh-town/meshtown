//! Integration tests of the checks: a local HTTP server and the exchange of `chk/` records between nodes.
//!
//! What it does: checks response parsing and byte counting on a real socket; that a stall after
//! 20 KB gives the "cuts the network" class; that a check record reaches the second node; that the records of two
//! nodes about one domain don't overwrite each other.
//! What it doesn't do: doesn't go to the internet, doesn't start a TLS server (the TLS step is checked by a live
//! run), doesn't check the time-based schedule.
//! Links: tests `meshtown_core::site_checks::http_min`, `meshtown_core::site_checks::reachability` and `meshtown_core::site_checks::checks`
//! through `meshtown_core::node`.

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::{self, NodeConfig, RunningNode};
use meshtown_core::site_checks::checks::{check_key, CheckView};
use meshtown_core::site_checks::http_min::{self, HttpGetRequest, HttpLimits};
use meshtown_core::site_checks::reachability::{
    self, CheckRecord, CheckVerdict, Steps, TcpStep, TlsStep, CHECK_RECORD_VERSION,
};
use meshtown_core::transport::net::PeerAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, Instant};

/// Starts a server that sends headers and `body_kb` kilobytes and then goes silent
/// if `stall` is set, otherwise closes the connection.
async fn serve(body_kb: usize, stall: bool) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let len = if stall { 1_000_000 } else { body_kb * 1024 };
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\n\r\n");
                if socket.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                let chunk = vec![b'x'; 1024];
                for _ in 0..body_kb {
                    if socket.write_all(&chunk).await.is_err() {
                        return;
                    }
                }
                if stall {
                    sleep(Duration::from_secs(30)).await;
                } else {
                    let _ = socket.shutdown().await;
                }
            });
        }
    });
    addr
}

fn limits(read_ms: u64) -> HttpLimits {
    HttpLimits {
        connect: Duration::from_secs(2),
        read: Duration::from_millis(read_ms),
        total: Duration::from_secs(10),
        max_bytes: 64 * 1024,
    }
}

fn request<'a>() -> HttpGetRequest<'a> {
    HttpGetRequest {
        host: "example.com",
        path: "/",
        accept: "*/*",
        user_agent: "meshtown-test",
    }
}

async fn introduce(a: &RunningNode, b: &RunningNode) {
    a.introduce(PeerAddr::new(b.id)).await;
}

fn sample(verdict_bytes: u32) -> CheckRecord {
    let steps = Steps {
        dns_ok: true,
        tcp: TcpStep::Ok,
        tls: TlsStep::Ok,
        http_status: 200,
        bytes_received: verdict_bytes,
        ..Steps::default()
    };
    let (verdict, reason) = reachability::classify(&steps);
    CheckRecord {
        v: CHECK_RECORD_VERSION,
        verdict,
        dns_ms: 10,
        tcp_ms: 20,
        tls_ms: 30,
        http_ms: 40,
        status: 200,
        bytes: verdict_bytes,
        stalled: false,
        at: 1_759_000_000,
        reason,
    }
}

/// Polls the results every 50 ms until the condition holds or the limit expires.
async fn wait_views(
    node: &RunningNode,
    limit: Duration,
    want: impl Fn(&[CheckView]) -> bool,
) -> Vec<CheckView> {
    let deadline = Instant::now() + limit;
    loop {
        let views = node.checks.check_views().await.unwrap();
        if want(&views) || Instant::now() >= deadline {
            return views;
        }
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_server_answers_and_bytes_are_counted() {
    let addr = serve(4, false).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let resp = http_min::http_get(&mut stream, request(), limits(2000))
        .await
        .unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body.len(), 4 * 1024);
    assert!(resp.raw_bytes > 4 * 1024);
    assert!(!resp.stalled);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stall_after_20_kb_is_network_blocked() {
    let addr = serve(20, true).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let resp = http_min::http_get(&mut stream, request(), limits(500))
        .await
        .unwrap();
    assert!(resp.stalled);
    let steps = Steps {
        dns_ok: true,
        tcp: TcpStep::Ok,
        tls: TlsStep::Ok,
        http_status: resp.status,
        bytes_received: resp.raw_bytes as u32,
        stalled: resp.stalled,
        ..Steps::default()
    };
    let (verdict, reason) = reachability::classify(&steps);
    assert_eq!(verdict, CheckVerdict::NetworkBlocked, "{reason}");
    assert!(reason.starts_with("freeze at 20 KB"), "{reason}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_record_syncs_between_peers() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    a.domains.add("example.com").await.unwrap();
    a.checks
        .publish("example.com", &sample(4096))
        .await
        .unwrap();

    let views = wait_views(&b, Duration::from_secs(20), |v| {
        v.iter()
            .any(|x| x.domain == "example.com" && x.record.is_some())
    })
    .await;
    let got = views
        .iter()
        .find(|x| x.domain == "example.com")
        .expect("the check record must reach the second node");
    assert_eq!(got.author, a.id);
    assert_eq!(got.record.as_ref().unwrap().verdict, CheckVerdict::Ok);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_authors_keep_own_records() {
    let key = NetworkKey::generate().unwrap();
    let a = node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    let b = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    introduce(&a, &b).await;

    a.domains.add("example.com").await.unwrap();
    a.checks
        .publish("example.com", &sample(4096))
        .await
        .unwrap();
    b.checks
        .publish("example.com", &sample(8192))
        .await
        .unwrap();

    let views = wait_views(&a, Duration::from_secs(20), |v| {
        v.iter().filter(|x| x.domain == "example.com").count() >= 2
    })
    .await;
    let mine: Vec<&CheckView> = views.iter().filter(|x| x.domain == "example.com").collect();
    assert_eq!(
        mine.len(),
        2,
        "the records of two nodes must not overwrite each other"
    );
    assert_ne!(mine[0].author, mine[1].author);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn try_begin_end_and_in_progress_mark_domain_busy() {
    let key = NetworkKey::generate().unwrap();
    let node = node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();

    assert!(node.checks.try_begin_check("example.com"));
    assert!(
        !node.checks.try_begin_check("example.com"),
        "a repeated try_begin for the same domain must return false"
    );
    assert!(node.checks.domains_in_progress().contains("example.com"));

    node.checks.end_check("example.com");
    assert!(!node.checks.domains_in_progress().contains("example.com"));
    assert!(
        node.checks.try_begin_check("example.com"),
        "after end the domain is free again"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn removed_record_stays_in_results_without_content() {
    // given: a node published a domain check result
    let node = single_node().await;
    node.checks
        .publish("example.com", &sample(4096))
        .await
        .unwrap();
    // when: the record is removed (a tombstone stays in the replica)
    node.domains
        .shared_replica()
        .del_own(&check_key("example.com"))
        .await
        .unwrap();
    // then: the node's record for the domain is in the results, but it has no content
    let views = node.checks.check_views().await.unwrap();
    assert_eq!(views.len(), 1, "the tombstone must stay in the results");
    assert_eq!(
        (views[0].domain.as_str(), views[0].author),
        ("example.com", node.id)
    );
    assert!(views[0].record.is_none(), "the tombstone has no content");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn record_of_foreign_version_is_shown_without_content() {
    // given: the document has a check record with a format version this node doesn't know
    let node = single_node().await;
    let future = CheckRecord {
        v: CHECK_RECORD_VERSION + 1,
        ..sample(4096)
    };
    let value = postcard::to_stdvec(&future).unwrap();
    node.domains
        .shared_replica()
        .set_own(&check_key("example.com"), Bytes::from(value))
        .await
        .unwrap();
    // when: we read the results
    let views = node.checks.check_views().await.unwrap();
    // then: the record is visible, but its content is not parsed
    assert_eq!(views.len(), 1);
    assert!(
        views[0].record.is_none(),
        "a foreign format version is not parsed"
    );
}

/// A node with no peers on loopback.
async fn single_node() -> RunningNode {
    let key = NetworkKey::generate().unwrap();
    node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap()
}
