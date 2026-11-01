//! The log line of an opened UDP session, thinned to one line per member and port in a window.

use std::time::Instant;

use super::PortServer;
use crate::transport::net::PeerId;

/// The line for a session of `label` on `port`; the tail counts the sessions held back since the last line.
pub(super) fn udp_opened_text(label: &str, port: u16, held: u64) -> String {
    let tail = match held {
        0 => String::new(),
        n => format!(" (+{n} more, not logged)"),
    };
    format!("{label}: port {port}/udp opened{tail}")
}

/// `Some(line)` when this member's sessions on `port` are due a line, else `None` (counted for the next one).
pub(super) fn udp_opened_line(
    server: &PortServer,
    peer: PeerId,
    port: u16,
    now: Instant,
) -> Option<String> {
    let held = server.udp_lines.due((peer, port), now)?;
    Some(udp_opened_text(
        &server.members.peer_label(peer),
        port,
        held,
    ))
}

#[cfg(test)]
#[path = "server_udp_log_tests.rs"]
mod tests;
