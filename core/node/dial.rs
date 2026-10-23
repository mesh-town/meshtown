//! Dialing a discovered member: starts an attempt, waits for the outcome, passes a failure on
//! and retries through `Redial` until the member is confirmed.
//!
//! What it does: handles discovered addresses (`on_addr_found`), dial failures (`on_dial_failed`), connection
//! up and down (`on_peer_up`/`on_peer_down`), wakes the node loop at the time of the next retry
//! (`sleep_until_due`/`dial_due_peers`).
//! What it doesn't do: doesn't decide whether it's already a member — asks `Members`; doesn't run the handshake
//! itself, only starts `handshake::initiate` from `dial_peer`.
//! Links: `node::event_loop::run_node_loop` holds one `PeerDialer` for the whole node loop;
//! `node::redial::Redial` is the state of the pauses, with no network.

use std::collections::HashSet;
use std::time::Duration;

use crate::transport::net::{self, NetStack, PeerAddr, PeerId, AUTH};
use tokio::sync::mpsc;
use tokio::time::{timeout, Instant};

use crate::members::access::view::Deny;
use crate::members::handshake::{self, RefusedByPeer};
use crate::members::membership::{self, MemberContext};
use crate::members::network_key::fill_random;
use crate::node::output::{say, short_peer_id};

use super::redial::{is_loud, Redial};

/// A dial failure (failed connection, 15 s timeout, handshake refusal) — the text of today's
/// log line instead of the line itself; `PeerDialer::on_dial_failed` prints it loudly or quietly.
pub(super) struct DialFailed {
    pub(super) peer: PeerId,
    pub(super) reason: String,
    /// The other side refused us with this reason, if it did.
    pub(super) refusal: Option<Deny>,
}

/// Dialing every known member that isn't connected yet: addresses from `on_addr_found`, failures from
/// the `DialFailed` channel, retries by the `Redial` deadline.
pub(super) struct PeerDialer {
    me: PeerId,
    stack: NetStack,
    ctx: MemberContext,
    redial: Redial,
    failed_tx: mpsc::UnboundedSender<DialFailed>,
}

impl PeerDialer {
    pub(super) fn new(
        me: PeerId,
        stack: NetStack,
        ctx: MemberContext,
        failed_tx: mpsc::UnboundedSender<DialFailed>,
    ) -> PeerDialer {
        PeerDialer {
            me,
            stack,
            ctx,
            redial: Redial::default(),
            failed_tx,
        }
    }

    /// Handles an address found outside (invite, cache, DHT): skips our own id and an address that no
    /// transport can dial, otherwise, if `Redial` allows, starts a dial right away
    /// (`NetStack::connect_addr` supplies the transport hints).
    pub(crate) fn on_addr_found(&mut self, addr: PeerAddr) {
        if addr.id == self.me {
            return;
        }
        // a bad address would overwrite in `Redial` the address found by another source; `hyper` dials by key
        if !self.stack.can_reach(&addr) {
            return;
        }
        if let Some((addr, attempt)) = self.redial.found(addr) {
            self.start_dial(addr, attempt);
        }
    }

    /// Starts a dial if `Members` lets it; otherwise the id is already a member or is already being dialed
    /// another way — the outcome will come with its own `Up`/`DialFailed`, not through this retry. The transport is already
    /// closed (the node is stopping or switching networks) — no dial is started at all: the connection
    /// won't come up anyway, and a log line would be false.
    fn start_dial(&mut self, addr: PeerAddr, attempt: u32) {
        if self.stack.is_closed() {
            return;
        }
        let peer = addr.id;
        if self.ctx.members.try_begin_dial(peer) {
            self.redial.started(peer);
            crate::node::output::spawn(dial_peer(
                self.stack.clone(),
                addr,
                self.ctx.clone(),
                attempt,
                self.failed_tx.clone(),
            ));
        } else {
            self.redial.up(peer);
        }
    }

    /// Handles a dial failure: the member is already confirmed another way (an incoming counter-call) —
    /// just cancels the retry without a line; otherwise counts the next attempt and pause, with a line
    /// on the first three attempts and on powers of two.
    pub(super) fn on_dial_failed(&mut self, f: DialFailed) {
        if self.ctx.members.is_member(f.peer) {
            self.redial.up(f.peer);
            return;
        }
        let (attempt, pause) = self.redial.failed(
            f.peer,
            Instant::now(),
            random_u32(),
            f.refusal == Some(Deny::Unlisted),
        );
        if is_loud(attempt) {
            say(
                "auth",
                format!(
                    "{}; attempt {attempt}, retry in {} s",
                    f.reason,
                    pause.as_secs()
                ),
            );
        }
    }

    pub(super) fn dial_due_peers(&mut self) {
        for (addr, attempt) in self.redial.take_due(Instant::now()) {
            self.start_dial(addr, attempt);
        }
    }

    pub(super) fn next_redial_at(&self) -> Option<Instant> {
        self.redial.next_due()
    }

    /// The member is confirmed (own `Up`) — cancels the retry.
    pub(super) fn on_peer_up(&mut self, peer: PeerId) {
        self.redial.up(peer);
    }

    /// The prune cut these members from the cache — a repeated dial forgets them.
    pub(super) fn forget_pruned_peers(&mut self, ids: &HashSet<PeerId>) {
        self.redial.forget(ids);
    }

    /// The connection to the member is broken (`Down`) — schedules the first retry and writes a line.
    /// The transport is already closed (node stop or `Cmd::Switch`) — schedules no retry and writes no
    /// line: there won't be a dial anyway, and a "redialing" line would be false — these `Down`s
    /// come from closing the stack, not from the member vanishing.
    pub(super) fn on_peer_down(&mut self, peer: PeerId, reason: &str) {
        if self.stack.is_closed() {
            return;
        }
        let pause = self.pause_after_down(peer, reason);
        say(
            "auth",
            format!(
                "{}: connection lost, redialing in {} s",
                short_peer_id(&peer),
                pause.as_secs()
            ),
        );
    }
}

impl PeerDialer {
    /// Schedules a retry after a member vanishes: a network change (`net::NET_CHANGED` at the end of the reason) -
    /// right away, otherwise the usual first pause. Returns the pause for the log line.
    fn pause_after_down(&mut self, peer: PeerId, reason: &str) -> Duration {
        let now = Instant::now();
        if reason.ends_with(net::NET_CHANGED) {
            return self.redial.down_now(peer, now);
        }
        self.redial.down(peer, now, random_u32())
    }
}

/// Sleeps until the time of the next retry; no deadline — never wakes (the `select!` branch
/// stays silent until `Redial` schedules the first retry).
pub(super) async fn sleep_until_due(due: Option<Instant>) {
    match due {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending::<()>().await,
    }
}

/// Four random bytes into a `u32`; a generator failure gives zero.
fn random_u32() -> u32 {
    let mut bytes = [0u8; 4];
    let _ = fill_random(&mut bytes);
    u32::from_le_bytes(bytes)
}

/// Clears the "dial in progress" mark and sends the failure to the `failed` channel with the same text;
/// `PeerDialer::on_dial_failed` decides whether to show it loudly or quietly. The order is the same in all three failure places
/// of `dial_peer`: first `end_dial`, then the send — a dial of the same id raised at that moment
/// will find `Members::dialing` already free, not busy.
fn report_failure(
    ctx: &MemberContext,
    failed: &mpsc::UnboundedSender<DialFailed>,
    peer: PeerId,
    reason: String,
    refusal: Option<Deny>,
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
fn say_initiator_success(peer: &PeerId, attempt: u32) {
    say(
        "auth",
        format!(
            "handshake with {}: success (I am the initiator{})",
            short_peer_id(peer),
            success_suffix(attempt)
        ),
    );
}

/// Dials the discovered member and runs the handshake as the initiator. A failure goes
/// not as a log line but through the `failed` channel with the same text (`report_failure`) —
/// `PeerDialer::on_dial_failed` decides, loudly or quietly.
async fn dial_peer(
    stack: NetStack,
    addr: PeerAddr,
    ctx: MemberContext,
    attempt: u32,
    failed: mpsc::UnboundedSender<DialFailed>,
) {
    let peer = addr.id;
    let connecting = stack.connect_addr(&addr, AUTH);
    let conn = match timeout(Duration::from_secs(15), connecting).await {
        Ok(Ok(conn)) => conn,
        Ok(Err(e)) => {
            let e = e.into_anyhow();
            report_failure(
                &ctx,
                &failed,
                peer,
                format!("no connection to {}: {e}", short_peer_id(&peer)),
                None,
            );
            return;
        }
        Err(_) => {
            report_failure(
                &ctx,
                &failed,
                peer,
                format!(
                    "no connection to {}: didn't finish within 15 s",
                    short_peer_id(&peer)
                ),
                None,
            );
            return;
        }
    };
    let result = handshake::initiate(&conn, &ctx).await;
    match result {
        Ok(()) => {
            ctx.members.end_dial(peer);
            say_initiator_success(&peer, attempt);
            membership::hold_conn_until_closed(conn, peer, ctx).await;
        }
        Err(e) => {
            report_failure(
                &ctx,
                &failed,
                peer,
                format!("handshake with {}: refused — {e:#}", short_peer_id(&peer)),
                e.downcast_ref::<RefusedByPeer>().map(|r| r.0),
            );
        }
    }
}

#[cfg(test)]
#[path = "dial_peer_tests.rs"]
mod peer_tests;
