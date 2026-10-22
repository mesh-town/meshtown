//! `[exit]` (on the exit) and `[exit-client]` (on the client) lines: every 5 s on an exit service connection, only if STREAM frames were sent during the period
//! — UDP bytes grow even when idle (keep-alive, ACK), STREAM frames don't.
//!
//! What it does: watches the exit connection, prints the bytes sent during the period and
//! the congestion statistics of the selected path.
//! What it doesn't do: it doesn't parse what is inside the tunnel and doesn't sum across rotations.
//! Links: `ExitService::handle` spawns `log_link_stats_while_open` with the tag `exit` before the `accept` loop,
//! `PeerConnPool::open_peer_stream` with the tag `exit-client` when a new connection opens; the statistics are
//! `PeerConn::congestion_text` and `PeerConn::link`; a connection without counters gives `(0, 0)` and no lines.

use std::time::Duration;

use std::sync::Arc;

use crate::net::{PeerConn, PeerId};
use tokio::time::sleep;

use crate::output::{say, short_peer_id};

const PERIOD: Duration = Duration::from_secs(5);

/// A line for the period: without STREAM frames (idle) there is no line.
fn exit_line(peer: &str, bytes: u64, stream_frames: u64, path: &str, congestion: &str) -> Option<String> {
    if stream_frames == 0 {
        return None;
    }
    let tail = if congestion.is_empty() { String::new() } else { format!("; {congestion}") };
    Some(format!("{peer}: sent {bytes} bytes in 5 s; path: {path}{tail}"))
}

fn sent_bytes_and_stream_frames(conn: &dyn PeerConn) -> (u64, u64) {
    let l = conn.link();
    (l.sent_bytes, l.stream_frames)
}

/// Prints a line with the tag `tag` once per `PERIOD` while the connection is alive.
pub(super) async fn log_link_stats_while_open(conn: Arc<dyn PeerConn>, peer: PeerId, tag: &'static str) {
    let peer_text = short_peer_id(&peer);
    let mut prev = sent_bytes_and_stream_frames(&*conn);
    loop {
        sleep(PERIOD).await;
        if conn.is_closed() {
            break;
        }
        let now = sent_bytes_and_stream_frames(&*conn);
        let bytes = now.0.saturating_sub(prev.0);
        let stream_frames = now.1.saturating_sub(prev.1);
        prev = now;
        let (path, congestion) = (conn.path_text(), conn.congestion_text());
        if let Some(line) = exit_line(&peer_text, bytes, stream_frames, &path, &congestion) {
            say(tag, line);
        }
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
