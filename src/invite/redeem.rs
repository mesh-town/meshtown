//! The one-time invite client: a newcomer presents the secret to members and gets the network key.
//!
//! What it does: takes a one-time transport with a random secret (the caller brings it up: `node::bind_ephemeral`), polls in a loop
//! the members from the string by the `meshtown/invite/1` protocol, proving it owns the node's permanent key (owner frame;
//! members of an older version get the older frame), repeats until the invite's deadline and decides which refusal is final; the transport is closed on any exit (`redeem_with`).
//! What it doesn't do: doesn't save the key, doesn't start the node and doesn't parse the invite string.
//! Links: frames are in `invite::wire`, the round decisions in `invite::redeem_round`, the server is `invite::server`; called by the CLI (`commands`) and `NodeCtl`.

use std::time::Duration;

use crate::net::{NetError, PeerAddr, PeerConn, PeerId, Transport, INVITE};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};

use super::redeem_round::{round_outcome, Frames, Mode, Outcome};
use super::wire::{parse_invite_reply, RedeemReply, REPLY_LEN};
use super::{InviteHead, INVITE_TTL_SECS};
use crate::net::NodeSecret;
use crate::net_mainline;
use crate::network_key::NetworkKey;
use crate::output::say;

/// The log line when the members turn out to run a version without member records.
const MEMBERS_OLD: &str = "members run an older version: joining without a member record; updated nodes will refuse this node until it joins by an invite from an updated node";
/// The connection time limit with one member.
const CONNECT_LIMIT: Duration = Duration::from_secs(10);
/// The frame exchange time limit on an established connection.
pub(crate) const EXCHANGE_LIMIT: Duration = Duration::from_secs(5);

/// The settings for joining by an invite.
#[derive(Clone, Debug)]
pub struct RedeemOpts {
    /// The relay mode; currently not read.
    pub use_relay: bool,
    pub first_pause: Duration,
    pub max_pause: Duration,
    pub deadline_unix: u64,
    pub unknown_rounds: u32,
}

impl RedeemOpts {
    /// The usual settings: a pause of 5 s, growing to 60 s, the deadline is the end of the invite's day, two "unknown" rounds.
    pub fn for_head(head: &InviteHead, use_relay: bool) -> Self {
        RedeemOpts {
            use_relay,
            first_pause: Duration::from_secs(5),
            max_pause: Duration::from_secs(60),
            deadline_unix: head.made_unix.saturating_add(INVITE_TTL_SECS),
            unknown_rounds: 2,
        }
    }
}

/// What the client is doing now (for the status line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    Asking(PeerId),
    Waiting(Duration),
}

/// The final refusal of joining.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedeemError {
    Used,
    Expired,
    Unknown,
    WrongNetwork,
    Excluded,
    Bind(String),
}

impl std::fmt::Display for RedeemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&text_of(self))
    }
}

impl std::error::Error for RedeemError {}

fn text_of(e: &RedeemError) -> String {
    match e {
        RedeemError::Used => "Another node has already joined with this invite — it is single-use. Ask a network member for a new one.".into(),
        RedeemError::Expired => "The invite was issued more than a day ago and is no longer valid. Ask a network member for a new one.".into(),
        RedeemError::Unknown => "Network members don't know this invite: it was revoked, it has expired or the string is damaged. Ask a network member for a new one.".into(),
        RedeemError::WrongNetwork => "A member answered with the key of another network — the invite was not accepted.".into(),
        RedeemError::Excluded => "This node's key is excluded from the network. The invite was not spent: it can be used on a node with a new key.".into(),
        RedeemError::Bind(why) => format!("couldn't set up a connection for joining: {why}"),
    }
}

/// Presents the invite to members until it gets a key, a final refusal or the deadline passes.
/// The transport `transport` (one-time, from `node::bind_ephemeral`) is closed both on success and on refusal
/// (`redeem_with`). Cancelling by dropping the future: `close` does not run, the transport is released via `Drop`
/// (peers will not see a close frame) — this is by design. `owner` is the node's permanent key: the members
/// write it into the membership list before they hand out the network key.
pub async fn redeem(
    head: &InviteHead,
    peers: &[PeerAddr],
    opts: &RedeemOpts,
    progress: &(dyn Fn(Progress) + Send + Sync),
    transport: Arc<dyn Transport>,
    owner: &NodeSecret,
) -> Result<NetworkKey, RedeemError> {
    redeem_with(transport, head, peers, opts, progress, owner).await
}

/// Owns the transport: the result of the rounds is kept, `close` comes before the return, with no `?` between them.
async fn redeem_with(
    t: Arc<dyn Transport>,
    head: &InviteHead,
    peers: &[PeerAddr],
    opts: &RedeemOpts,
    progress: &(dyn Fn(Progress) + Send + Sync),
    owner: &NodeSecret,
) -> Result<NetworkKey, RedeemError> {
    let frames = Frames::new(head, t.me(), owner);
    let result = rounds(&*t, head, peers, opts, progress, &frames).await;
    t.close().await;
    result
}

async fn rounds(
    t: &dyn Transport,
    head: &InviteHead,
    peers: &[PeerAddr],
    opts: &RedeemOpts,
    progress: &(dyn Fn(Progress) + Send + Sync),
    frames: &Frames,
) -> Result<NetworkKey, RedeemError> {
    let mut pause = opts.first_pause;
    let mut unknown = 0u32;
    let mut mode = Mode::Owner;
    while !remaining(opts.deadline_unix).is_zero() {
        match run_round(t, head, peers, opts, progress, (frames, mode)).await {
            Outcome::Key(key) => return Ok(key),
            Outcome::Refused(why) => return Err(why),
            Outcome::Legacy => {
                say("invite", MEMBERS_OLD);
                mode = Mode::Legacy;
                continue;
            }
            Outcome::AllUnknown => unknown += 1,
            Outcome::Retry => unknown = 0,
        }
        if unknown > 0 && unknown >= opts.unknown_rounds {
            return Err(RedeemError::Unknown);
        }
        progress(Progress::Waiting(pause));
        sleep(pause.min(remaining(opts.deadline_unix))).await;
        pause = next_pause(pause, opts.max_pause);
    }
    Err(RedeemError::Expired)
}

/// How much is left until the invite's deadline.
fn remaining(deadline_unix: u64) -> Duration {
    Duration::from_secs(deadline_unix.saturating_sub(net_mainline::unix_now()))
}

async fn run_round(
    t: &dyn Transport,
    head: &InviteHead,
    peers: &[PeerAddr],
    opts: &RedeemOpts,
    progress: &(dyn Fn(Progress) + Send + Sync),
    (frames, mode): (&Frames, Mode),
) -> Outcome {
    let mut answers = Vec::with_capacity(peers.len());
    for addr in peers {
        progress(Progress::Asking(addr.id));
        let left = remaining(opts.deadline_unix);
        let answer = timeout(left, ask(t, addr, frames.of(mode))).await.ok().flatten();
        answers.push(answer);
        if round_outcome(&answers, head.network_id, mode).decisive() {
            break;
        }
    }
    round_outcome(&answers, head.network_id, mode)
}

/// One attempt: a connection, checking the other side's id, a frame there and back; any failure is `None`.
async fn ask(t: &dyn Transport, addr: &PeerAddr, frame: &[u8]) -> Option<RedeemReply> {
    let conn = timeout(CONNECT_LIMIT, t.connect(addr, INVITE)).await.ok()?.ok()?;
    let reply = ask_on(&*conn, addr.id, frame).await;
    conn.close(0, b"done");
    reply
}

async fn ask_on(conn: &dyn PeerConn, expected: PeerId, frame: &[u8]) -> Option<RedeemReply> {
    if !peer_is(expected, conn.peer()) {
        return None;
    }
    timeout(EXCHANGE_LIMIT, redeem_round_trip(conn, frame)).await.ok()?.ok()
}

async fn redeem_round_trip(conn: &dyn PeerConn, frame: &[u8]) -> Result<RedeemReply> {
    let pair = conn.open().await.map_err(NetError::into_anyhow)?;
    let (mut send, mut recv) = (pair.send, pair.recv);
    send.write_all(frame).await?;
    send.shutdown().await?;
    let mut buf = [0u8; REPLY_LEN];
    recv.read_exact(&mut buf).await?;
    parse_invite_reply(&buf).ok_or_else(|| anyhow!("reply of an unrecognized kind"))
}

/// Whether the member at the other end is the one we called.
fn peer_is(expected: PeerId, got: PeerId) -> bool {
    expected == got
}

/// The next pause between rounds: twice as long, but no more than the ceiling.
fn next_pause(cur: Duration, max: Duration) -> Duration {
    cur.saturating_mul(2).min(max)
}

#[cfg(test)]
#[path = "redeem_tests.rs"]
mod tests;
