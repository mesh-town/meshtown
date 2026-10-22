//! Tests of the `mem` connection (`conn.rs`, `stream.rs`): windows, half-close, packet size, parallel streams.
//! Closing, accounting and protocol violations are in `conn_close_tests.rs`.

use crate::transport::net::mem::conn::{MemConn, STREAM_WINDOW_BYTES};
use crate::transport::net::mem::frame::Frame;
use crate::transport::net::mem::{MemNet, MemPath, MEM_MAX_PACKET};
use crate::transport::net::{
    BoxRecv, BoxSend, NodeSecret, PacketPath, PeerConn, PeerId, StreamPair,
};
use bytes::Bytes;
use std::future::Future;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;
use tokio::time::timeout;

mod window {
    use super::*;

    #[tokio::test]
    async fn writing_stalls_after_one_window_when_nobody_reads() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let _held = accept(&b).await;

        let written = fill_until_stalled(&mut out.send).await;

        assert_eq!(
            written, STREAM_WINDOW_BYTES,
            "the write must stop exactly at the window"
        );
    }

    #[tokio::test]
    async fn reading_half_a_window_lets_the_writer_go_on() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let mut inc = accept(&b).await;
        fill_until_stalled(&mut out.send).await;

        read_exactly(&mut inc.recv, STREAM_WINDOW_BYTES / 2).await;

        let more = within("write after the read", out.send.write_all(b"more")).await;
        more.expect("write after the read");
    }

    #[tokio::test]
    async fn several_windows_of_data_arrive_intact_and_in_order() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let mut inc = accept(&b).await;
        let data = pattern(3 * STREAM_WINDOW_BYTES, 1);

        let ((), got) = within_secs(
            20,
            "transfer of three windows",
            both(
                write_all(&mut out.send, &data),
                read_exactly(&mut inc.recv, data.len()),
            ),
        )
        .await;

        assert!(
            got == data,
            "read something else: {} bytes out of {}",
            got.len(),
            data.len()
        );
    }

    #[tokio::test]
    async fn a_stream_nobody_reads_does_not_hold_up_another() {
        let (a, b) = conns();
        let mut blocked = open(&a).await;
        let mut free = open(&a).await;
        let _unread = accept(&b).await;
        let mut free_in = accept(&b).await;
        fill_until_stalled(&mut blocked.send).await;
        let data = pattern(4 << 20, 9);

        let ((), got) = within_secs(
            20,
            "4 MiB over the second stream",
            both(
                write_all(&mut free.send, &data),
                read_to_end(&mut free_in.recv),
            ),
        )
        .await;

        assert!(
            got == data,
            "read something else: {} bytes out of {}",
            got.len(),
            data.len()
        );
    }
}

mod dropped_receiver {
    use super::*;

    #[tokio::test]
    async fn the_sender_writes_four_windows_without_stopping() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let StreamPair { send: _keep, recv } = accept(&b).await;
        drop(recv);

        let data = vec![3u8; 4 * STREAM_WINDOW_BYTES];

        let all = out.send.write_all(&data);

        within_secs(10, "write of 4·W", all)
            .await
            .expect("write of 4·W");
    }

    #[tokio::test]
    async fn dropping_it_releases_a_stalled_writer() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let StreamPair { send: _keep, recv } = accept(&b).await;
        fill_until_stalled(&mut out.send).await;

        drop(recv);

        let data = vec![3u8; 2 * STREAM_WINDOW_BYTES];
        let more = out.send.write_all(&data);
        within_secs(10, "write after the reset", more)
            .await
            .expect("write");
    }
}

mod half_close {
    use super::*;

    #[tokio::test]
    async fn shutdown_without_data_gives_the_far_side_zero_bytes_and_eof() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let mut inc = accept(&b).await;

        within("shutdown", out.send.shutdown())
            .await
            .expect("shutdown");

        assert_eq!(read_to_end(&mut inc.recv).await.len(), 0);
    }

    #[tokio::test]
    async fn shutdown_after_data_delivers_the_data_then_eof() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let mut inc = accept(&b).await;
        out.send.write_all(b"tail").await.expect("write");

        within("shutdown", out.send.shutdown())
            .await
            .expect("shutdown");

        assert_eq!(read_to_end(&mut inc.recv).await, b"tail");
    }

    #[tokio::test]
    async fn writing_after_shutdown_is_broken_pipe() {
        let (a, _b) = conns();
        let mut out = open(&a).await;
        out.send.shutdown().await.expect("shutdown");

        let err = out
            .send
            .write(b"x")
            .await
            .expect_err("write after shutdown");

        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn a_second_shutdown_changes_nothing() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let mut inc = accept(&b).await;

        out.send.shutdown().await.expect("first shutdown");
        out.send.shutdown().await.expect("second shutdown");

        assert_eq!(read_to_end(&mut inc.recv).await.len(), 0);
        let open_after = timeout(Duration::from_millis(200), b.closed())
            .await
            .is_err();
        assert!(open_after, "the repeated shutdown closed the connection");
        assert_eq!(b.close_info(), None);
    }

    #[tokio::test]
    async fn dropping_the_sender_without_shutdown_is_eof_for_the_far_side() {
        let (a, b) = conns();
        let StreamPair { send, recv: _recv } = open(&a).await;
        let mut inc = accept(&b).await;

        drop(send);

        assert_eq!(read_to_end(&mut inc.recv).await.len(), 0);
    }

    #[tokio::test]
    async fn the_other_direction_keeps_working_after_one_side_shuts_down() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let mut inc = accept(&b).await;
        out.send.shutdown().await.expect("shutdown");

        inc.send
            .write_all(b"reply")
            .await
            .expect("write of the reply");

        assert_eq!(read_exactly(&mut out.recv, 5).await, b"reply");
    }
}

#[path = "conn_parallel_tests.rs"]
mod packets_and_parallelism;

#[path = "conn_diag_tests.rs"]
mod diag_defaults;

// ---- helpers ----

fn peer(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// A line of two ends: `a` is node 1, `b` is node 2.
fn paths() -> (MemPath, MemPath) {
    let net = MemNet::new();
    MemPath::pair(&net.hub, (peer(1), 1), (peer(2), 2))
}

/// Two connections on one line: A dials, B accepts.
fn conns() -> (Arc<MemConn>, Arc<MemConn>) {
    let (pa, pb) = paths();
    (
        MemConn::spawn(Arc::new(pa), true),
        MemConn::spawn(Arc::new(pb), false),
    )
}

fn pattern(len: usize, seed: usize) -> Vec<u8> {
    (0..len)
        .map(|i| i.wrapping_mul(31).wrapping_add(seed) as u8)
        .collect()
}

async fn within_secs<T>(secs: u64, what: &str, fut: impl Future<Output = T>) -> T {
    let done = timeout(Duration::from_secs(secs), fut).await;
    done.unwrap_or_else(|_| panic!("{what}: did not finish within {secs} s"))
}

async fn within<T>(what: &str, fut: impl Future<Output = T>) -> T {
    within_secs(5, what, fut).await
}

/// `tokio::join!` of two futures as one future (to wrap in a time limit).
async fn both<A, B>(first: impl Future<Output = A>, second: impl Future<Output = B>) -> (A, B) {
    tokio::join!(first, second)
}

async fn open(conn: &Arc<MemConn>) -> StreamPair {
    let opened = within("open", conn.open()).await;
    opened.unwrap_or_else(|e| panic!("open returned an error: {e}"))
}

async fn accept(conn: &Arc<MemConn>) -> StreamPair {
    let accepted = within("accept", conn.accept()).await;
    accepted.unwrap_or_else(|e| panic!("accept returned an error: {e}"))
}

async fn write_all(send: &mut BoxSend, data: &[u8]) {
    send.write_all(data).await.expect("write");
    send.shutdown().await.expect("shutdown");
}

async fn read_exactly(recv: &mut BoxRecv, n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    within_secs(20, "read", recv.read_exact(&mut buf))
        .await
        .expect("read");
    buf
}

async fn read_to_end(recv: &mut BoxRecv) -> Vec<u8> {
    let mut buf = Vec::new();
    within_secs(20, "read to the end", recv.read_to_end(&mut buf))
        .await
        .expect("read to the end");
    buf
}

/// Writes in 16 KiB chunks until the next write hangs for 200 ms; returns what was accepted.
async fn fill_until_stalled(send: &mut BoxSend) -> usize {
    let chunk = [7u8; 16 * 1024];
    let mut total = 0;
    while let Ok(n) = timeout(Duration::from_millis(200), send.write(&chunk)).await {
        total += n.expect("write");
        assert!(
            total <= 8 * STREAM_WINDOW_BYTES,
            "the write did not stop after {total} bytes"
        );
    }
    total
}
