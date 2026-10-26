//! Outcome of one dial: the failure message, who refused the handshake, and the log lines.
//!
//! What it does: carries a failure from `dial_peer` to `PeerDialer::on_dial_failed` (`DialFailed`), classifies
//! a handshake error (`DialRefusal::of`), reports failure and success.
//! What it doesn't do: no pauses and no decisions — see `dial` and `redial`.

use anyhow::Error;
use tokio::sync::mpsc;

use crate::members::access::view::Deny;
use crate::members::handshake::{RefusedByPeer, RefusedByUs};
use crate::members::membership::MemberContext;
use crate::node::output::{say, short_peer_id};
use crate::transport::net::PeerId;

/// A dial failure (failed connection, 15 s timeout, handshake refusal) — the text of today's
/// log line instead of the line itself; `PeerDialer::on_dial_failed` prints it loudly or quietly.
pub(super) struct DialFailed {
    pub(super) peer: PeerId,
    pub(super) reason: String,
    /// Who refused the handshake, if anyone did.
    pub(super) refusal: DialRefusal,
}

/// Who refused a handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DialRefusal {
    NotRefused,
    /// The other side refused us with this reason.
    ByPeer(Deny),
    /// Our own list refused the other side with this reason.
    ByUs(Deny),
}

impl DialRefusal {
    /// Reads the refusal from a handshake error; our own refusal wins over the peer's.
    pub(super) fn of(e: &Error) -> Self {
        if let Some(r) = e.downcast_ref::<RefusedByUs>() {
            return DialRefusal::ByUs(r.0);
        }
        match e.downcast_ref::<RefusedByPeer>() {
            Some(r) => DialRefusal::ByPeer(r.0),
            None => DialRefusal::NotRefused,
        }
    }
}

/// Clears the "dial in progress" mark and sends the failure to the `failed` channel with the same text;
/// `PeerDialer::on_dial_failed` decides whether to show it loudly or quietly. The order is the same in all three failure places
/// of `dial_peer`: first `end_dial`, then the send — a dial of the same id raised at that moment
/// will find `Members::dialing` already free, not busy.
pub(super) fn report_failure(
    ctx: &MemberContext,
    failed: &mpsc::UnboundedSender<DialFailed>,
    peer: PeerId,
    reason: String,
    refusal: DialRefusal,
) {
    ctx.members.end_dial(peer);
    let _ = failed.send(DialFailed {
        peer,
        reason,
        refusal,
    });
}

/// Suffix for the handshake success line: empty on the first attempt, otherwise the attempt number.
fn success_suffix(attempt: u32) -> String {
    if attempt <= 1 {
        return String::new();
    }
    format!(", attempt {attempt}")
}

/// Logs a successful handshake in which this node was the initiator.
pub(super) fn say_initiator_success(peer: &PeerId, attempt: u32) {
    say(
        "auth",
        format!(
            "handshake with {}: success (I am the initiator{})",
            short_peer_id(peer),
            success_suffix(attempt)
        ),
    );
}

#[cfg(test)]
#[path = "dial_outcome_tests.rs"]
mod tests;
