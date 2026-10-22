//! The server of the `meshtown/invite/1` protocol: checks the one-time secret and hands out the network key.
//!
//! What it does: reads a frame on each connection. Owner request (kind 3): see `invite::server_owner` — checks the
//! newcomer's proof of owning its permanent key, asks the membership view, writes the claim, the `admit` record
//! and the mark, then answers with the key. HOLD: saves the issue entry locally at the issuing node's request.
//! An older client's request (kind 1) is refused without spending the invite.
//! What it doesn't do: writes to the log neither the secret, nor its hash, nor the key.
//! Links: plugged in `node::start::network::build_stack` via `invite_service`; the issue journal is `invite::store`.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;

use super::hold_wire::{hold_reply, parse_hold, HOLD_LEN, HOLD_SKEW_SECS, HOLD_FRAME_KIND};
use super::store::{issue_record, Ledger};
use super::wire::{encode_invite_reply, RedeemCode, OWNER_FRAME_KIND, OWNER_REQUEST_LEN, REPLY_LEN, REQUEST_LEN};
use super::head::hash_hex_of;
use super::INVITE_TTL_SECS;
use crate::access::view::AccessView;
use crate::net_mainline;
use crate::domains::DomainList;
use crate::net::{BoxFuture, BoxSend, NetError, PeerConn, PeerId, ServiceHandler};
use crate::network_key::NetworkKey;
use crate::output::{say, short_peer_id};

/// How many presentations are served at once.
pub const MAX_REDEEMS: usize = 4;
/// The time limit for waiting for a request.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// The time limit of a verdict with writing the mark.
pub(super) const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);
/// The wait between one's own claim and the comparison: during it the claim reaches the rival.
pub(crate) const CLAIM_SETTLE: Duration = Duration::from_secs(2);
/// The time limit for waiting for the connection to close after the answer.
const CLOSE_WAIT: Duration = Duration::from_secs(5);

/// The handler of incoming invite presentations.
#[derive(Clone)]
pub struct InviteServer {
    pub(super) key: NetworkKey,
    pub(super) list: Arc<OnceLock<DomainList>>,
    pub(super) access: AccessView,
    slots: Arc<Semaphore>,
    pub(super) gate: Arc<tokio::sync::Mutex<()>>,
}

impl InviteServer {
    /// The server for this network's key; `list` is filled after the document is opened; `access` says
    /// whether this node may let a newcomer's permanent key in.
    pub fn new(key: NetworkKey, list: Arc<OnceLock<DomainList>>, access: AccessView) -> Self {
        InviteServer {
            key,
            list,
            access,
            slots: Arc::new(Semaphore::new(MAX_REDEEMS)),
            gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// The answer to a frame: the only choice by frame kind; `transport` is the caller's one-time id.
    async fn answer(&self, frame: &Frame, transport: PeerId) -> [u8; REPLY_LEN] {
        let who = short_peer_id(&transport);
        match frame {
            Frame::Legacy(req) => legacy_reply(req[0], &who),
            Frame::Owner(req) => self.owner_reply(req, transport, &who).await,
            Frame::Hold(req) => self.hold_reply(req, &who).await,
        }
    }

    async fn hold_reply(&self, req: &[u8; HOLD_LEN], who: &str) -> [u8; REPLY_LEN] {
        let code = match timeout(ANSWER_TIMEOUT, self.decide_hold(req)).await {
            Ok(code) => code,
            Err(_) => RedeemCode::Internal,
        };
        log_hold(who, code);
        hold_reply(code)
    }

    /// The code for a "save" request: signature and kind, future time, deadline, the list, one's own copy of the entry.
    async fn decide_hold(&self, req: &[u8; HOLD_LEN]) -> RedeemCode {
        let Some(hold) = parse_hold(&self.key, req) else {
            return RedeemCode::BadRequest;
        };
        let now_unix = net_mainline::unix_now();
        if hold.made_unix > now_unix.saturating_add(HOLD_SKEW_SECS) {
            return RedeemCode::BadRequest;
        }
        if now_unix >= hold.made_unix.saturating_add(INVITE_TTL_SECS) {
            return RedeemCode::Expired;
        }
        let Some(list) = self.list.get() else {
            return RedeemCode::NotReady;
        };
        let rec = issue_record(hold.made_unix, None);
        match Ledger::of(list).issue(&hash_hex_of(&hold.hash), &rec).await {
            Ok(()) => RedeemCode::Ok,
            Err(_) => RedeemCode::Internal,
        }
    }

    /// Reads a frame and answers; `permit == None` — no free slots, the answer is `Busy` in a v1 frame.
    async fn serve(&self, conn: &dyn PeerConn, permit: Option<OwnedSemaphorePermit>) -> Result<()> {
        let (mut send, frame) = timeout(READ_TIMEOUT, read_frame(conn))
            .await
            .map_err(|_| anyhow!("request did not arrive in time"))??;
        let who = short_peer_id(&conn.peer());
        let reply = match permit {
            Some(_held) => self.answer(&frame, conn.peer()).await,
            None => busy_reply(&who),
        };
        send.write_all(&reply).await?;
        send.shutdown().await?;
        let _ = timeout(CLOSE_WAIT, conn.closed()).await;
        Ok(())
    }
}

/// The frame that came first on the service: an older admission request (33 bytes), an owner request (129)
/// or HOLD (73).
enum Frame {
    Legacy([u8; REQUEST_LEN]),
    Owner([u8; OWNER_REQUEST_LEN]),
    Hold([u8; HOLD_LEN]),
}

/// 33 bytes; by the first one reads the rest of a HOLD or an owner frame under the same time limit of the caller.
async fn read_frame(conn: &dyn PeerConn) -> Result<(BoxSend, Frame)> {
    let pair = conn.accept().await.map_err(NetError::into_anyhow)?;
    let (send, mut recv) = (pair.send, pair.recv);
    let mut buf = [0u8; REQUEST_LEN];
    recv.read_exact(&mut buf).await?;
    let frame = match buf[0] {
        HOLD_FRAME_KIND => Frame::Hold(extend(&buf, &mut recv).await?),
        OWNER_FRAME_KIND => Frame::Owner(extend(&buf, &mut recv).await?),
        _ => Frame::Legacy(buf),
    };
    Ok((send, frame))
}

/// A longer frame: the 33 bytes already read and the rest from the stream.
async fn extend<const N: usize>(head: &[u8; REQUEST_LEN], recv: &mut (impl AsyncReadExt + Unpin)) -> Result<[u8; N]> {
    let mut full = [0u8; N];
    full[..REQUEST_LEN].copy_from_slice(head);
    recv.read_exact(&mut full[REQUEST_LEN..]).await?;
    Ok(full)
}

/// A request of an older program: refused without a claim, the invite stays unspent.
fn legacy_reply(kind: u8, who: &str) -> [u8; REPLY_LEN] {
    say("invite", format!("invite refused for {who}: request kind {kind} from an older program; the invite was not spent"));
    encode_invite_reply(RedeemCode::BadRequest, None)
}

fn busy_reply(who: &str) -> [u8; REPLY_LEN] {
    log_refused(who, RedeemCode::Busy);
    encode_invite_reply(RedeemCode::Busy, None)
}

pub(super) fn log_refused(who: &str, code: RedeemCode) {
    say("invite", format!("invite refused for {who}: {code:?}"));
}

fn log_hold(who: &str, code: RedeemCode) {
    if code == RedeemCode::Ok {
        say("invite", format!("invite entry accepted for storage from {who}"));
        return;
    }
    say("invite", format!("request from {who} to save an entry refused: {code:?}"));
}

/// The invite handler for `NetStack`: `InviteServer` itself as a `ServiceHandler`.
pub fn invite_service(
    key: NetworkKey,
    list: Arc<OnceLock<DomainList>>,
    access: AccessView,
) -> Arc<dyn ServiceHandler> {
    Arc::new(InviteServer::new(key, list, access))
}

impl ServiceHandler for InviteServer {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let this = self.clone();
        Box::pin(async move {
            let permit = this.slots.clone().try_acquire_owned().ok();
            if let Err(e) = this.serve(&*conn, permit).await {
                say("invite", format!("{}: presentation interrupted: {e:#}", short_peer_id(&conn.peer())));
            }
        })
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
