//! Close log of a `hyper` session: one line per session, from the side that won the close.

use crate::node::output::say;
use crate::transport::net::id::PeerId;

/// Who closed the session: `Us` — `close_local`, `Peer` — `close_remote` (peer's Close, a break, a write failure).
#[derive(Clone, Copy)]
pub(crate) enum Side {
    Us,
    Peer,
}

impl Side {
    fn text(self) -> &'static str {
        match self {
            Side::Us => "by us",
            Side::Peer => "not by us",
        }
    }
}

/// Builds the log line; the reason is taken as is, unreadable bytes are replaced.
pub(crate) fn close_line(
    peer: &PeerId,
    session: u64,
    side: Side,
    code: u32,
    reason: &[u8],
) -> String {
    let head = &peer.to_string()[..8];
    let reason = String::from_utf8_lossy(reason);
    format!(
        "session {head} #{session:x} closed {}: code {code}: {reason}",
        side.text()
    )
}

/// Writes the close line to the log.
pub(crate) fn say_close(peer: &PeerId, session: u64, side: Side, code: u32, reason: &[u8]) {
    say("hyper", close_line(peer, session, side, code, reason));
}

/// Builds the line that ties a session to the id of its control stream.
pub(crate) fn control_stream_line(peer: &PeerId, session: u64, stream_id: Option<u32>) -> String {
    let head = &peer.to_string()[..8];
    let id = stream_id.map_or("unknown".to_string(), |id| id.to_string());
    format!("session {head} #{session:x} control stream id {id}")
}

/// Writes the control stream line to the log.
pub(crate) fn say_control_stream(peer: &PeerId, session: u64, stream_id: Option<u32>) {
    say("hyper", control_stream_line(peer, session, stream_id));
}

#[cfg(test)]
#[path = "close_log_tests.rs"]
mod tests;
