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

#[cfg(test)]
#[path = "close_log_tests.rs"]
mod tests;
