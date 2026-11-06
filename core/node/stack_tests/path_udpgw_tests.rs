//! Tests of `open_through_exits` (`path.rs`) for the UDP gateway target: connections share one exit stream,
//! other targets still get a stream per request.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::path_busy_tests::config_with;
use crate::exit;
use crate::proxy::path::*;
use crate::proxy::serve::ProxyConfig;
use crate::transport::net::facade::test_support::{pid, within};
use crate::transport::net::{BoxFuture, PeerConn, ServiceHandler, StreamPair};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

/// An exit that answers every request with OK and keeps the streams open; `hits` counts the requests.
#[derive(Default)]
struct Holding {
    hits: Arc<AtomicUsize>,
    held: Arc<Mutex<Vec<StreamPair>>>,
}

impl ServiceHandler for Holding {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let (hits, held) = (self.hits.clone(), self.held.clone());
        Box::pin(async move {
            while let Ok(mut pair) = conn.accept().await {
                let mut head = [0u8; 2];
                if pair.recv.read_exact(&mut head).await.is_err() {
                    continue;
                }
                let mut tail = vec![0u8; usize::from(head[1]) + 2];
                let _ = pair.recv.read_exact(&mut tail).await;
                hits.fetch_add(1, Ordering::SeqCst);
                let _ = pair
                    .send
                    .write_all(&[exit::EXIT_PROTOCOL_VERSION, exit::EXIT_CODE_OK])
                    .await;
                held.lock().unwrap().push(pair);
            }
        })
    }
}

async fn config_holding() -> (ProxyConfig, Arc<AtomicUsize>) {
    let exit = Arc::new(Holding::default());
    let hits = exit.hits.clone();
    let cfg = config_with(&[], || exit.clone() as Arc<dyn ServiceHandler>).await;
    (cfg, hits)
}

async fn open(cfg: &ProxyConfig, host: &str, port: u16) -> ExitLink {
    let res = within(
        "open_through_exits",
        open_through_exits(cfg, &[pid(2)], host, port),
    )
    .await;
    res.ok().expect("the exit opens the stream")
}

/// given: an exit that keeps its streams open; when the gateway target is opened four times, then the exit
/// sees one request.
#[tokio::test]
async fn four_gateway_connects_share_one_exit_stream() {
    let (cfg, hits) = config_holding().await;

    let mut links = Vec::new();
    for _ in 0..4 {
        links.push(open(&cfg, exit::UDPGW_HOST, exit::UDPGW_PORT).await);
    }

    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

/// given: the same exit; when an ordinary target is opened twice, then the exit sees two requests.
#[tokio::test]
async fn ordinary_target_still_opens_a_stream_per_request() {
    let (cfg, hits) = config_holding().await;

    let mut links = Vec::new();
    for _ in 0..2 {
        links.push(open(&cfg, "example.com", 80).await);
    }

    assert_eq!(hits.load(Ordering::SeqCst), 2);
}
