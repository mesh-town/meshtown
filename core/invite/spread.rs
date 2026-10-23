//! Collecting invite holders: online members who confirmed they saved the entry.
//!
//! What it does: after the issue entry is written, asks up to `HOLD_ASK_MAX` members in parallel to save
//! their own copy (a HOLD frame over the INVITE service), waits no longer than `HOLD_WAIT` and for no more than
//! `HOLDERS_WANTED` confirmations; lays out the string's entries: holders, the creator, the rest of the list.
//! What it doesn't do: returns no errors (no confirmation — not a holder); knows no address hints
//! of the holders — dialing is by key; writing the copy is `invite::server`.
//! Links: called by `invite::Issued::commit`; the frames are in `invite::hold_wire`.

use std::collections::BTreeSet;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;
use tokio::time::{timeout_at, Instant};

use super::hold_wire::{parse_hold_reply, HOLD_LEN};
use super::wire::{RedeemCode, REPLY_LEN};
use super::MAX_INVITE_PEERS;
use crate::transport::net::{NetStack, PeerAddr, PeerId, INVITE};

/// How many confirmations are enough: we don't wait for more.
pub const HOLDERS_WANTED: usize = 2;
/// How many members we ask: more won't fit into the string.
pub const HOLD_ASK_MAX: usize = MAX_INVITE_PEERS - 1;
/// How long we wait for confirmations in total (the tray client's ceiling is 2 s for the whole request).
pub const HOLD_WAIT: Duration = Duration::from_millis(1500);

/// Whom to ask: members without myself, by key, the first `HOLD_ASK_MAX`.
pub fn candidates(members: &[PeerId], me: PeerId) -> Vec<PeerId> {
    let mut out: Vec<PeerId> = members.iter().copied().filter(|p| *p != me).collect();
    out.sort_by_key(|p| *p.as_bytes());
    out.dedup();
    out.truncate(HOLD_ASK_MAX);
    out
}

/// The entries after the creator: connected members by key, then the other listed members by key;
/// without myself and without repeats.
pub fn rest(connected: &[PeerId], admitted: &BTreeSet<PeerId>, me: PeerId) -> Vec<PeerId> {
    let by_key = |mut ids: Vec<PeerId>| {
        ids.sort_by_key(|p| *p.as_bytes());
        ids
    };
    let linked = by_key(connected.iter().copied().filter(|p| *p != me).collect());
    let offline = by_key(
        admitted
            .iter()
            .copied()
            .filter(|p| *p != me && !linked.contains(p))
            .collect(),
    );
    let mut out = linked;
    out.extend(offline);
    out.dedup();
    out
}

/// Asks everyone in parallel; exits on `HOLDERS_WANTED` confirmations, running out of
/// candidates or `HOLD_WAIT`; holders gathered before the deadline are kept.
pub async fn gather(
    stack: &NetStack,
    frame: [u8; HOLD_LEN],
    candidates: Vec<PeerId>,
) -> Vec<PeerId> {
    let deadline = Instant::now() + HOLD_WAIT;
    let mut set = JoinSet::new();
    for peer in candidates {
        let stack = stack.clone();
        crate::node::output::spawn_in(&mut set, async move {
            (peer, ask_hold(stack, peer, frame).await)
        });
    }
    let mut holders = Vec::new();
    while holders.len() < HOLDERS_WANTED {
        match timeout_at(deadline, set.join_next()).await {
            Ok(Some(Ok((peer, true)))) => holders.push(peer),
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => break,
        }
    }
    holders
}

/// One member: a connection, checking the id, a frame there and back; any failure is `false`.
async fn ask_hold(stack: NetStack, peer: PeerId, frame: [u8; HOLD_LEN]) -> bool {
    let Ok(conn) = stack.connect(peer, INVITE).await else {
        return false;
    };
    let ok = conn.peer() == peer && hold_round_trip(&*conn, &frame).await.unwrap_or(false);
    conn.close(0, b"done");
    ok
}

async fn hold_round_trip(
    conn: &dyn crate::transport::net::PeerConn,
    frame: &[u8; HOLD_LEN],
) -> anyhow::Result<bool> {
    let pair = conn
        .open()
        .await
        .map_err(crate::transport::net::NetError::into_anyhow)?;
    let (mut send, mut recv) = (pair.send, pair.recv);
    send.write_all(frame).await?;
    send.shutdown().await?;
    let mut buf = [0u8; REPLY_LEN];
    recv.read_exact(&mut buf).await?;
    Ok(parse_hold_reply(&buf) == Some(RedeemCode::Ok))
}

/// The order of the string's entries: holders (by key), the creator (with hints), the rest of the list.
/// Repeats by key and anything over `MAX_INVITE_PEERS` are dropped by assembling the string.
pub fn ordered(holders: &[PeerId], me: &PeerAddr, rest: &[PeerId]) -> Vec<PeerAddr> {
    let first = holders.iter().copied().map(PeerAddr::new);
    let rest = rest.iter().copied().map(PeerAddr::new);
    first
        .chain(std::iter::once(me.clone()))
        .chain(rest)
        .collect()
}

#[cfg(test)]
#[path = "spread_tests.rs"]
mod tests;
