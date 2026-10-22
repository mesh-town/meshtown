//! Volume probe: how many bytes pass over an already established connection.
//!
//! What it does: once per interval sends the peer empty data of growing size
//! (16 KiB / 256 KiB, plus 1 MiB to the chosen exit) and waits for the length to be acknowledged; answers other peers' probes.
//! What it does not do: it does not choose a path and does not decide whether a probe is allowed — it takes the path for the log
//! ready-made from `PeerConn` (`path_text`, `congestion_text`); it does not work with a raw transport connection.
//! Links: `membership::hold_conn_until_closed` starts `answer_probes` and `send_probes_while_open` on every verified connection.

use std::sync::atomic::{AtomicUsize, Ordering};
use portable_atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::net::{NetError, PeerConn, PeerId, StreamPair};
use anyhow::{ensure, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};

use crate::output::{say, short_peer_id};

/// Probe sizes in turn: 16 KiB, 256 KiB, 1 MiB.
pub const PROBE_SIZES_BYTES: [usize; 3] = [16384, 262144, 1048576];
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PROBE_BYTES: usize = 1048576;
const PROBE_CHUNK_BYTES: usize = 16384;

/// Volume probe settings for one connection. `interval_secs` is a live handle (seconds behind
/// an atomic): re-read on every turn of `send_probes_while_open`, without recreating the task.
#[derive(Debug, Clone)]
pub struct ProbeSettings {
    pub first_delay: Duration,
    pub interval_secs: Arc<AtomicU64>,
    pub ok_probe_count: Arc<AtomicUsize>,
    /// Current list of the node's chosen exits (the same source as `RouteCtx`): 1 MiB is sent
    /// only to whoever is in it; decided on every turn, the exit is changed on the fly.
    pub exits: tokio::sync::watch::Receiver<Vec<PeerId>>,
    /// Send 1 MiB to all peers (`probe.full_cycle`), as before the probe was narrowed.
    pub full_cycle: bool,
}

/// Size of the next probe by turn number: to the exit (or on a full cycle) — 16 KiB /
/// 256 KiB / 1 MiB in a loop, to the rest — only 16 KiB / 256 KiB.
pub fn probe_size_bytes(turn: usize, to_exit: bool, full_cycle: bool) -> usize {
    let cycle = if to_exit || full_cycle { 3 } else { 2 };
    PROBE_SIZES_BYTES[turn % cycle]
}

/// Sends a probe of the given size and waits for an acknowledgment of the same length.
pub async fn send_probe(conn: &dyn PeerConn, size: usize) -> Result<Duration> {
    let started = Instant::now();
    let StreamPair { mut send, mut recv } = conn.open().await.map_err(NetError::into_anyhow)?;
    send.write_all(&(size as u32).to_le_bytes()).await?;
    let chunk = [0u8; PROBE_CHUNK_BYTES];
    let mut sent = 0usize;
    while sent < size {
        let n = chunk.len().min(size - sent);
        send.write_all(&chunk[..n]).await?;
        sent += n;
    }
    send.shutdown().await?;
    let mut ack = [0u8; 4];
    recv.read_exact(&mut ack).await?;
    let acked = u32::from_le_bytes(ack) as usize;
    ensure!(acked == size, "acknowledged length {acked} instead of {size}");
    Ok(started.elapsed())
}

/// Receives other peers' probes on this connection and acknowledges the received length.
pub async fn answer_probes(conn: Arc<dyn PeerConn>, peer: PeerId) {
    loop {
        let StreamPair { mut send, mut recv } = match conn.accept().await {
            Ok(streams) => streams,
            Err(_) => break,
        };
        tokio::spawn(async move {
            let mut len_buf = [0u8; 4];
            if recv.read_exact(&mut len_buf).await.is_err() {
                return;
            }
            let size = u32::from_le_bytes(len_buf) as usize;
            if size > MAX_PROBE_BYTES {
                return;
            }
            let started = Instant::now();
            let mut buf = vec![0u8; PROBE_CHUNK_BYTES];
            let mut received = 0usize;
            while received < size {
                let n = buf.len().min(size - received);
                if recv.read_exact(&mut buf[..n]).await.is_err() {
                    return;
                }
                received += n;
            }
            if send.write_all(&len_buf).await.is_err() {
                return;
            }
            let _ = send.shutdown().await;
            say(
                "probe",
                format!(
                    "<- {}: {} bytes received in {} ms",
                    short_peer_id(&peer),
                    size,
                    started.elapsed().as_millis()
                ),
            );
        });
    }
}

/// Result of one probe: success with the acknowledgment time, a transfer error or a timeout.
type ProbeOutcome = Result<Result<Duration, anyhow::Error>, tokio::time::error::Elapsed>;

/// `[probe]` line for one turn: path and congestion statistics are the common tail, the outcome is
/// per branch.
fn probe_line(peer: &str, size: usize, path: &str, congestion: &str, outcome: ProbeOutcome) -> String {
    match outcome {
        Ok(Ok(elapsed)) => format!(
            "-> {peer}: {size} bytes, acknowledged in {} ms, path: {path}; {congestion}",
            elapsed.as_millis()
        ),
        Ok(Err(e)) => format!("-> {peer}: {size} bytes — error: {e}; path: {path}; {congestion}"),
        Err(_) => format!(
            "-> {peer}: {size} bytes — no acknowledgment within 30 s; path: {path}; {congestion}"
        ),
    }
}

/// Sends the peer probes of growing size in a loop once per interval while the connection is alive.
pub async fn send_probes_while_open(conn: Arc<dyn PeerConn>, peer: PeerId, settings: ProbeSettings) {
    sleep(settings.first_delay).await;
    for turn in 0usize.. {
        if conn.is_closed() {
            break;
        }
        let to_exit = settings.exits.borrow().contains(&peer);
        let size = probe_size_bytes(turn, to_exit, settings.full_cycle);
        let path = conn.path_text();
        let outcome = timeout(PROBE_TIMEOUT, send_probe(&*conn, size)).await;
        if matches!(outcome, Ok(Ok(_))) {
            settings.ok_probe_count.fetch_add(1, Ordering::Relaxed);
        }
        let congestion = conn.congestion_text();
        say("probe", probe_line(&short_peer_id(&peer), size, &path, &congestion, outcome));
        sleep(Duration::from_secs(settings.interval_secs.load(Ordering::Relaxed))).await;
    }
}

#[cfg(test)]
#[path = "transfer_probe_tests.rs"]
mod tests;
