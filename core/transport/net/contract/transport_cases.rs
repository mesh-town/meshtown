//! Helpers of the "Transport → …" cases: nodes, connection A→B, stream echo, bulk exchange.
//! Each step runs under the `must_finish` limit; the bulk exchange under `must_finish_within`.

use crate::transport::net::contract::{
    must_finish, must_finish_within, Harness, SEED_A, SEED_B, STEP,
};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::link::LinkInfo;
use crate::transport::net::service::{ServiceId, AUTH};
use crate::transport::net::stream::{BoxRecv, BoxSend, StreamPair};
use crate::transport::net::transport::{PeerConn, Transport};
use rand::{Rng, SeedableRng};
use rand_pcg::Pcg64Mcg;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

/// Time within which the counterpart sees the close and an unserved service gets a refusal.
pub(crate) const CLOSE_WITHIN: Duration = Duration::from_secs(2);
/// Time limit of the "bulk" step: a debug build; `hyper` alone takes tens of seconds, longer under machine load; nextest kills a test at 150 s, `mem` takes seconds.
const VOLUME_LIMIT: Duration = Duration::from_secs(100);
/// Window in which `accept` must not return an unserved service.
const QUIET: Duration = Duration::from_millis(300);
/// Poll period of `is_closed()` in the "close is visible to the counterpart" case.
const POLL: Duration = Duration::from_millis(10);

/// How many streams the "100 streams" case opens and how many bytes it sends into each.
pub(crate) const STREAMS: usize = 100;
pub(crate) const STREAM_BYTES: usize = 64 << 10;

/// Volume of one stream in the "bulk" case and the size of a write chunk.
const VOLUME_BYTES: usize = 64 << 20;
const CHUNK: usize = 64 << 10;
const VOLUME_SEED: u64 = 0x6d65_7368;

pub(crate) const PING: &[u8] = b"ping";
pub(crate) const PONG: &[u8] = b"pong";

/// A live connection: `a` is the handle of the dialer, `b` of the acceptor.
pub(crate) struct ConnPair {
    pub a: Arc<dyn PeerConn>,
    pub b: Arc<dyn PeerConn>,
}

/// Length and blake3 of a byte stream.
pub(crate) struct Digest {
    pub len: u64,
    pub hash: blake3::Hash,
}

/// Two nodes of one environment: A and B; both have `serve([AUTH, PORT])` called.
pub(crate) async fn transport_pair(h: &impl Harness) -> (Arc<dyn Transport>, Arc<dyn Transport>) {
    let a = must_finish(h.node(SEED_A)).await;
    let b = must_finish(h.node(SEED_B)).await;
    (a, b)
}

/// `from` connects to `to` on `svc` at its `local_addr()`, `to` accepts; both steps run at once.
pub(crate) async fn connect_between(
    from: &Arc<dyn Transport>,
    to: &Arc<dyn Transport>,
    svc: ServiceId,
) -> (ServiceId, ConnPair) {
    let addr = to.local_addr();
    let (dialed, accepted) = tokio::join!(
        must_finish(from.connect(&addr, svc)),
        must_finish(to.accept())
    );
    let a = dialed.unwrap_or_else(|e| panic!("connect gave no connection: {e}"));
    let (got, b) = accepted.expect("accept is closed, but a connection was expected");
    (got, ConnPair { a, b })
}

/// A new stream on the handle.
pub(crate) async fn open_stream(conn: &Arc<dyn PeerConn>) -> StreamPair {
    let opened = must_finish(conn.open()).await;
    opened.unwrap_or_else(|e| panic!("open gave no stream: {e}"))
}

/// The next incoming stream of the handle.
pub(crate) async fn accept_stream(conn: &Arc<dyn PeerConn>) -> StreamPair {
    let accepted = must_finish(conn.accept()).await;
    accepted.unwrap_or_else(|e| panic!("accept gave no stream: {e}"))
}

/// Writes `data` and half-closes the sending half.
async fn write_finish(send: &mut BoxSend, data: &[u8]) {
    must_finish(async {
        send.write_all(data).await.expect("write to the stream");
        send.shutdown().await.expect("shutdown of the stream");
    })
    .await;
}

/// Reads the receiving half to the end of the stream.
async fn read_all(recv: &mut BoxRecv) -> Vec<u8> {
    let mut got = Vec::new();
    let read = must_finish(recv.read_to_end(&mut got)).await;
    read.expect("read from the stream");
    got
}

/// A writes "ping" and half-closes; B reads to the end and answers "pong". Returns (what B heard, A's reply).
pub(crate) async fn ping_pong(pair: &ConnPair) -> (Vec<u8>, Vec<u8>) {
    let mut from_a = open_stream(&pair.a).await;
    write_finish(&mut from_a.send, PING).await;
    let mut at_b = accept_stream(&pair.b).await;
    let heard = read_all(&mut at_b.recv).await;
    write_finish(&mut at_b.send, PONG).await;
    let answer = read_all(&mut from_a.recv).await;
    (heard, answer)
}

/// The `link()` info of the dialer after the "ping"/"pong" exchange over the new connection A→B.
pub(crate) async fn link_after_exchange(h: &impl Harness) -> LinkInfo {
    let (a, b) = transport_pair(h).await;
    let (_, pair) = connect_between(&a, &b, AUTH).await;
    ping_pong(&pair).await;
    pair.a.link()
}

/// Echo server: every incoming stream of the handle is returned to the sender as is, until it is half-closed.
pub(crate) fn spawn_echo(conn: Arc<dyn PeerConn>) -> JoinHandle<()> {
    crate::transport::net::task_label::spawn(async move {
        while let Ok(stream) = conn.accept().await {
            crate::transport::net::task_label::spawn(echo_stream(stream));
        }
    })
}

async fn echo_stream(stream: StreamPair) {
    let StreamPair { mut send, mut recv } = stream;
    let _ = tokio::io::copy(&mut recv, &mut send).await;
    let _ = send.shutdown().await;
}

/// Pseudo-random bytes, different for different `seed`.
fn payload(seed: u64, len: usize) -> Vec<u8> {
    let mut data = vec![0u8; len];
    Pcg64Mcg::seed_from_u64(seed).fill_bytes(&mut data);
    data
}

/// Opens a stream, sends `len` bytes into it, half-closes and reads the echo; tells whether it matches what was sent.
async fn round_trip(conn: Arc<dyn PeerConn>, seed: u64, len: usize) -> bool {
    let data = payload(seed, len);
    // The time limit for the whole exchange is set by `echo_mismatches` (shared by the hundred streams), hence no `must_finish`.
    let StreamPair { mut send, mut recv } = conn.open().await.expect("open gave a stream");
    let write = async {
        send.write_all(&data).await.expect("write to the stream");
        send.shutdown().await.expect("shutdown of the stream");
    };
    let read = async {
        let mut got = Vec::new();
        recv.read_to_end(&mut got)
            .await
            .expect("read from the stream");
        got
    };
    let ((), echoed) = tokio::join!(write, read);
    echoed == data
}

/// Numbers of the streams, out of `count` concurrent ones, whose echo did not match what was sent.
pub(crate) async fn echo_mismatches(
    conn: &Arc<dyn PeerConn>,
    count: usize,
    len: usize,
    limit: Duration,
) -> Vec<usize> {
    let tasks: Vec<_> = (0..count)
        .map(|i| crate::transport::net::task_label::spawn(round_trip(conn.clone(), i as u64, len)))
        .collect();
    let matched = must_finish_within(limit, join_all(tasks)).await;
    let bad = matched.iter().enumerate().filter(|(_, ok)| !**ok);
    bad.map(|(i, _)| i).collect()
}

async fn join_all(tasks: Vec<JoinHandle<bool>>) -> Vec<bool> {
    let mut done = Vec::with_capacity(tasks.len());
    for task in tasks {
        done.push(task.await.expect("stream task panicked"));
    }
    done
}

/// Sends 64 MiB of pseudo-random chunks and computes the blake3 of what was sent.
async fn send_volume(mut send: BoxSend) -> Digest {
    let mut rng = Pcg64Mcg::seed_from_u64(VOLUME_SEED);
    let mut hasher = blake3::Hasher::new();
    let mut chunk = vec![0u8; CHUNK];
    for _ in 0..VOLUME_BYTES / CHUNK {
        rng.fill_bytes(&mut chunk);
        hasher.update(&chunk);
        send.write_all(&chunk)
            .await
            .expect("write of the bulk data");
    }
    send.shutdown().await.expect("shutdown of the bulk stream");
    Digest {
        len: VOLUME_BYTES as u64,
        hash: hasher.finalize(),
    }
}

/// Reads the stream to the end and computes the length and blake3 of what was received.
async fn recv_volume(mut recv: BoxRecv) -> Digest {
    let mut hasher = blake3::Hasher::new();
    let mut len = 0u64;
    let mut buf = vec![0u8; CHUNK];
    while let Some(n) = read_some(&mut recv, &mut buf).await {
        hasher.update(&buf[..n]);
        len += n as u64;
    }
    Digest {
        len,
        hash: hasher.finalize(),
    }
}

/// The next chunk of bytes of the stream; `None` means the end of the stream.
async fn read_some(recv: &mut BoxRecv, buf: &mut [u8]) -> Option<usize> {
    let n = recv.read(buf).await.expect("read of the bulk data");
    Some(n).filter(|n| *n > 0)
}

/// A sends one 64 MiB stream, B reads it to the end; returns (sent, received).
pub(crate) async fn transfer_volume(pair: &ConnPair) -> (Digest, Digest) {
    let sender = async {
        let StreamPair { send, recv: _kept } = open_stream(&pair.a).await;
        send_volume(send).await
    };
    let receiver = async {
        let StreamPair { send: _kept, recv } = accept_stream(&pair.b).await;
        recv_volume(recv).await
    };
    must_finish_within(VOLUME_LIMIT, async { tokio::join!(sender, receiver) }).await
}

/// Refusal: `connect` returned an error or the connection closed within `CLOSE_WITHIN`.
pub(crate) async fn rejected(outcome: NetResult<Arc<dyn PeerConn>>) -> bool {
    let Ok(conn) = outcome else { return true };
    tokio::time::timeout(CLOSE_WITHIN, conn.closed())
        .await
        .is_ok()
}

/// Within a short window `accept` at `at` does not return `svc` (silence is also correct).
pub(crate) async fn assert_not_accepted(at: &Arc<dyn Transport>, svc: ServiceId) {
    let Ok(accepted) = tokio::time::timeout(QUIET, at.accept()).await else {
        return;
    };
    let (got, _conn) = accepted.expect("accept is closed, but the transport is alive");
    assert_ne!(got, svc, "accept returned the unserved service {svc:?}");
}

/// Within `STEP` the handle starts answering `is_closed() == true` (polled every `POLL`).
pub(crate) async fn becomes_closed(conn: &Arc<dyn PeerConn>) -> bool {
    let polling = async {
        while !conn.is_closed() {
            tokio::time::sleep(POLL).await;
        }
    };
    tokio::time::timeout(STEP, polling).await.is_ok()
}

/// The `Closed` error as the result of an operation on a closed connection.
pub(crate) fn is_closed_error<T>(result: &NetResult<T>) -> bool {
    matches!(result, Err(NetError::Closed))
}
