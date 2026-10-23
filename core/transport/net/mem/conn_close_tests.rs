//! Tests of the `mem` connection (`conn.rs`): closing, path break, the last handle, violations, stream ids, accounting.
//! Windows, half-close and parallel streams are in `conn_tests.rs`.

use crate::transport::net::mem::conn::{MemConn, CODE_PROTOCOL, STREAM_WINDOW_BYTES};
use crate::transport::net::mem::frame::Frame;
use crate::transport::net::mem::{MemNet, MemPath, MEM_MAX_PACKET};
use crate::transport::net::{
    BoxRecv, BoxSend, NetError, NetResult, NodeSecret, PacketPath, PathKind, PeerConn, PeerId,
    StreamPair,
};
use bytes::Bytes;
use std::future::Future;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};

mod close_with_a_code {
    use super::*;

    #[tokio::test]
    async fn the_far_side_learns_the_code_and_the_reason() {
        let (a, b) = conns();

        a.close(7, b"bye");

        within("closed at B", b.closed()).await;
        assert_eq!(b.close_info(), Some((7, Bytes::from_static(b"bye"))));
    }

    #[tokio::test]
    async fn the_closing_side_knows_its_own_code_at_once() {
        let (a, _b) = conns();

        a.close(7, b"bye");

        assert_eq!(a.close_info(), Some((7, Bytes::from_static(b"bye"))));
    }

    #[tokio::test]
    async fn both_sides_report_closed() {
        let (a, b) = conns();

        a.close(7, b"bye");

        within("closed at A", a.closed()).await;
        within("closed at B", b.closed()).await;
    }

    #[tokio::test]
    async fn open_and_accept_on_the_closing_side_are_closed() {
        let (a, _b) = conns();
        a.close(7, b"bye");

        assert_closed(a.open().await, "open at the closing side");
        assert_closed(a.accept().await, "accept at the closing side");
    }

    #[tokio::test]
    async fn open_and_accept_on_the_far_side_are_closed() {
        let (a, b) = conns();
        a.close(7, b"bye");
        within("closed at B", b.closed()).await;

        assert_closed(b.open().await, "open at the counterpart");
        assert_closed(b.accept().await, "accept at the counterpart");
    }

    #[tokio::test]
    async fn a_waiting_accept_wakes_up_closed() {
        let (a, b) = conns();
        let waiting = crate::transport::net::task_label::spawn({
            let b = b.clone();
            async move { b.accept().await.err() }
        });
        sleep(Duration::from_millis(50)).await;

        a.close(7, b"bye");

        let err = within("pending accept", waiting)
            .await
            .expect("accept task");
        assert!(matches!(err, Some(NetError::Closed)), "error: {err:?}");
    }

    #[tokio::test]
    async fn a_long_reason_is_cut_to_a_packet_and_does_not_break_the_close() {
        let (a, b) = conns();

        a.close(9, &vec![b'x'; 100_000]);

        within("closed at B", b.closed()).await;
        let (code, reason) = b.close_info().expect("B must have a close code");
        assert_eq!(code, 9);
        assert_eq!(
            reason.len(),
            MEM_MAX_PACKET - 5,
            "the reason is truncated to a packet without the header"
        );
    }
}

mod streams_when_closed {
    use super::*;

    #[tokio::test]
    async fn an_unfinished_stream_reads_its_buffer_then_connection_aborted() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        out.send.write_all(b"partial").await.expect("write");
        let mut inc = accept(&b).await;
        a.close(1, b"");
        within("closed at B", b.closed()).await;

        let got = read_exactly(&mut inc.recv, 7).await;
        let err = inc
            .recv
            .read(&mut [0u8; 1])
            .await
            .expect_err("read without Fin");

        assert_eq!(got, b"partial");
        assert_eq!(err.kind(), ErrorKind::ConnectionAborted);
    }

    #[tokio::test]
    async fn a_finished_stream_reads_its_buffer_then_eof() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        out.send.write_all(b"done").await.expect("write");
        out.send.shutdown().await.expect("shutdown");
        let mut inc = accept(&b).await;
        a.close(1, b"");
        within("closed at B", b.closed()).await;

        let mut got = Vec::new();
        inc.recv
            .read_to_end(&mut got)
            .await
            .expect("read to the end after Fin");

        assert_eq!(got, b"done");
    }

    #[tokio::test]
    async fn a_waiting_read_ends_with_connection_aborted() {
        let (a, b) = conns();
        let _out = open(&a).await;
        let mut inc = accept(&b).await;
        let waiting =
            crate::transport::net::task_label::spawn(
                async move { inc.recv.read(&mut [0u8; 1]).await },
            );
        sleep(Duration::from_millis(50)).await;

        a.close(1, b"");

        let res = within("waiting read", waiting).await.expect("read task");
        assert_eq!(
            res.expect_err("read without Fin").kind(),
            ErrorKind::ConnectionAborted
        );
    }

    #[tokio::test]
    async fn a_write_after_the_close_is_broken_pipe() {
        let (a, _b) = conns();
        let mut out = open(&a).await;
        a.close(1, b"");

        let err = out.send.write(b"x").await.expect_err("write after close");

        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn a_write_on_the_far_side_after_the_close_is_broken_pipe() {
        let (a, b) = conns();
        let _out = open(&a).await;
        let mut inc = accept(&b).await;
        a.close(1, b"");
        within("closed at B", b.closed()).await;

        let err = inc
            .send
            .write(b"x")
            .await
            .expect_err("write after the other side's close");

        assert_eq!(err.kind(), ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn a_stalled_write_ends_with_broken_pipe_when_the_conn_closes() {
        let (a, b) = conns();
        let mut out = open(&a).await;
        let _held = accept(&b).await;
        fill_until_stalled(&mut out.send).await;
        let stalled =
            crate::transport::net::task_label::spawn(
                async move { out.send.write(&[1u8; 1024]).await },
            );
        sleep(Duration::from_millis(50)).await;

        b.close(1, b"");

        let res = within("stalled write", stalled).await.expect("write task");
        assert_eq!(
            res.expect_err("write after close").kind(),
            ErrorKind::BrokenPipe
        );
    }
}

mod lost_path {
    use super::*;

    #[tokio::test]
    async fn cutting_the_path_closes_both_sides() {
        let (a, b, path_a) = conns_with_path_of_a();

        path_a.close();

        within("closed at A", a.closed()).await;
        within("closed at B", b.closed()).await;
    }

    #[tokio::test]
    async fn a_far_end_that_vanishes_closes_the_conn() {
        let (conn, raw) = conn_with_raw(true);

        drop(raw);

        within("closed after the counterpart disappeared", conn.closed()).await;
    }
}

mod last_handle {
    use super::*;

    #[tokio::test]
    async fn dropping_the_conn_alone_closes_the_far_side_with_code_zero() {
        let (a, b) = conns();

        drop(a);

        within("closed at B", b.closed()).await;
        assert_eq!(b.close_info(), Some((0, Bytes::new())));
    }

    #[tokio::test]
    async fn a_live_stream_half_keeps_the_conn_open_after_the_conn_is_dropped() {
        let (a, b) = conns();
        let stream = open(&a).await;
        drop(a);

        let still_open = timeout(Duration::from_millis(200), b.closed())
            .await
            .is_err();

        assert!(still_open, "the connection closed while a stream was alive");
        drop(stream);
    }

    #[tokio::test]
    async fn dropping_the_last_stream_half_closes_with_code_zero() {
        let (a, b) = conns();
        let StreamPair { send, recv } = open(&a).await;
        drop(a);
        drop(send);

        drop(recv);

        within("closed at B", b.closed()).await;
        assert_eq!(b.close_info().map(|(code, _)| code), Some(0));
    }
}

#[path = "conn_violation_tests.rs"]
mod violations_and_accounting;

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
    let (a, b, _path_a) = conns_with_path_of_a();
    (a, b)
}

/// The same, plus the shared path of A — to break it from outside.
fn conns_with_path_of_a() -> (Arc<MemConn>, Arc<MemConn>, Arc<MemPath>) {
    let (pa, pb) = paths();
    let pa = Arc::new(pa);
    let a = MemConn::spawn(pa.clone(), true);
    (a, MemConn::spawn(Arc::new(pb), false), pa)
}

/// A connection (dialing or accepting); the other end of the line is a "raw" path without `MemConn`.
fn conn_with_raw(dialer: bool) -> (Arc<MemConn>, MemPath) {
    let (pa, raw) = paths();
    (MemConn::spawn(Arc::new(pa), dialer), raw)
}

async fn within_secs<T>(secs: u64, what: &str, fut: impl Future<Output = T>) -> T {
    let done = timeout(Duration::from_secs(secs), fut).await;
    done.unwrap_or_else(|_| panic!("{what}: did not finish within {secs} s"))
}

async fn within<T>(what: &str, fut: impl Future<Output = T>) -> T {
    within_secs(5, what, fut).await
}

fn assert_closed(res: NetResult<StreamPair>, what: &str) {
    let err = res.err();
    assert!(
        matches!(err, Some(NetError::Closed)),
        "{what}: expected Closed, got {err:?}"
    );
}

async fn open(conn: &Arc<MemConn>) -> StreamPair {
    let opened = within("open", conn.open()).await;
    opened.unwrap_or_else(|e| panic!("open returned an error: {e}"))
}

async fn accept(conn: &Arc<MemConn>) -> StreamPair {
    let accepted = within("accept", conn.accept()).await;
    accepted.unwrap_or_else(|e| panic!("accept returned an error: {e}"))
}

async fn read_exactly(recv: &mut BoxRecv, n: usize) -> Vec<u8> {
    let mut buf = vec![0u8; n];
    within("read", recv.read_exact(&mut buf))
        .await
        .expect("read");
    buf
}

/// Writes in 16 KiB chunks until the next write hangs for 200 ms.
async fn fill_until_stalled(send: &mut BoxSend) {
    let chunk = [7u8; 16 * 1024];
    let mut total = 0;
    while let Ok(n) = timeout(Duration::from_millis(200), send.write(&chunk)).await {
        total += n.expect("write");
        assert!(
            total <= 8 * STREAM_WINDOW_BYTES,
            "the write did not stop after {total} bytes"
        );
    }
}
