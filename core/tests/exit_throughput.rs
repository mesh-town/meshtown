//! Exit throughput measurement on loopback: a single stream
//! of 64 MiB through node A's SOCKS5 and exit B, `#[ignore]` — run separately, not in the battery.
//!
//! What it does: two real nodes on loopback (`common::pair_with_static_exit`), behind exit B is
//! its own server with a deterministic non-uniform body (a 64 KiB block from xorshift with a fixed
//! seed, repeated). The client reads the body and checks every chunk against the reference by offset
//! (stricter than a hash: it catches ordering too), and prints one line `throughput_mbit=<MiB·8/s>` based on the time
//! from the first byte of the body to the last.
//! What it doesn't do: it doesn't measure the network between machines, there is no RTT on loopback. Links: `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use meshtown::socks5;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use common::{pair_with_static_exit, wait_exit_route, DOMAIN};

const BLOCK: usize = 64 * 1024;
const TOTAL: usize = 64 * 1024 * 1024;

/// A block of the reference body: xorshift64 with a fixed seed.
fn block() -> Vec<u8> {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut out = Vec::with_capacity(BLOCK);
    while out.len() < BLOCK {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Server: answers any request with a header and `TOTAL` bytes of the repeating block.
async fn serve_big() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let body = std::sync::Arc::new(block());
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {TOTAL}\r\n\r\n");
                if socket.write_all(head.as_bytes()).await.is_err() {
                    return;
                }
                for _ in 0..TOTAL / BLOCK {
                    if socket.write_all(&body).await.is_err() {
                        return;
                    }
                }
                let _ = socket.shutdown().await;
            });
        }
    });
    addr
}

/// Checks a chunk of the body starting at offset `off` against the repeating reference block.
fn check(expected: &[u8], off: usize, chunk: &[u8]) {
    let mut pos = 0;
    while pos < chunk.len() {
        let at = (off + pos) % BLOCK;
        let take = (chunk.len() - pos).min(BLOCK - at);
        assert_eq!(&chunk[pos..pos + take], &expected[at..at + take], "body diverges at {}", off + pos);
        pos += take;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "speed measurement, run by hand: --ignored --nocapture"]
async fn exit_throughput() {
    // given: A with an exit at B, behind B a server with 64 MiB of a known body
    let site = serve_big().await;
    let (a, b) = pair_with_static_exit(site).await;
    let route = wait_exit_route(&a, Duration::from_secs(20)).await;
    assert_eq!(route, meshtown::route::Route::Exits(vec![b.id]));

    // when: the client pulls the body through node A's SOCKS5
    let proxy = a.socks_addr.expect("the SOCKS5 server must be up");
    let mut stream = socks5::client_connect(proxy, DOMAIN, 80).await.unwrap();
    let req = format!("GET / HTTP/1.1\r\nHost: {DOMAIN}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.unwrap();
    let expected = block();
    let (mut head, mut got, mut start) = (Vec::new(), 0usize, None::<Instant>);
    let mut buf = vec![0u8; 128 * 1024];
    let mut end = Instant::now();
    while got < TOTAL {
        let n = stream.read(&mut buf).await.unwrap();
        assert!(n > 0, "stream broke off at {got} bytes of {TOTAL}");
        let mut chunk = &buf[..n];
        if start.is_none() {
            head.extend_from_slice(chunk);
            let Some(pos) = head.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            chunk = &head[pos + 4..];
            start = Some(Instant::now());
        }
        // then: every chunk matches the reference by offset
        check(&expected, got, chunk);
        got += chunk.len();
        end = Instant::now();
    }
    assert_eq!(got, TOTAL, "body length");
    let secs = end.duration_since(start.unwrap()).as_secs_f64();
    println!("throughput_mbit={:.1}", (TOTAL as f64 / 1048576.0) * 8.0 / secs);
}
