//! Handshake on the network secret — checks that both members know the same key.
//!
//! What it does: the initiator and the responder exchange one-time numbers and tags
//! computed with the handshake key; a tag mismatch drops the connection.
//! What it doesn't do: it negotiates nothing beyond the fact of a shared secret and the node name — no addresses,
//! no topics. The name rides as a tail after the tag: a frame of "2 length bytes + UTF-8"; an old version neither
//! sends nor reads the tail.
//! Links: `node::dial_peer` calls `initiate`, `AuthService` (the stack's AUTH service) calls `respond`;
//! on success both sides add the member through `membership::Members`.

use std::sync::Arc;
use std::time::Duration;

use crate::transport::net::{
    read_to_end_limited, BoxFuture, NetError, PeerConn, PeerId, ServiceHandler, StreamPair,
};
use anyhow::{bail, ensure, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

use crate::members::access::view::{Deny, Side, ACCESS_CLOSE_CODE};
use crate::members::membership::{self, MemberContext, MemberEvent};
use crate::members::network_key::fill_random;
use crate::members::words::{trim_and_validate_node_name, MAX_NAME_BYTES};
use crate::node::output::{say, short_peer_id, NET_NAME};

/// ALPN of the handshake protocol.
pub const AUTH_ALPN: &[u8] = b"meshtown/auth/1";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// The handshake tail is a name frame (2 + up to 256 bytes); the rest is room for fields of future versions,
/// which this version skips.
const MAX_TAIL_BYTES: usize = 1024;

/// Name frame: an empty name or one longer than `MAX_NAME_BYTES` means "no name" (`[0, 0]`); otherwise the length
/// as `u16` big-endian (2 bytes) and the bytes themselves. It neither trims nor validates the name — the node's own name has already
/// passed validation (the default or `trim_and_validate_node_name`).
pub fn name_frame(name: &str) -> Vec<u8> {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_NAME_BYTES {
        return vec![0, 0];
    }
    let mut out = Vec::with_capacity(2 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(bytes);
    out
}

/// Parses the handshake tail into the peer's name. Returns `None` on anything odd (shorter than 2 bytes, length 0 or
/// above `MAX_NAME_BYTES`, fewer bytes than promised, not UTF-8, rejected by `trim_and_validate_node_name`) —
/// the handshake still succeeds, the peer is simply treated as nameless. Bytes after the frame are not
/// looked at at all (room for future fields).
pub fn parse_name_tail(tail: &[u8]) -> Option<String> {
    let len = u16::from_be_bytes(tail.get(0..2)?.try_into().unwrap()) as usize;
    if len == 0 || len > MAX_NAME_BYTES {
        return None;
    }
    let name_bytes = tail.get(2..2 + len)?;
    let s = std::str::from_utf8(name_bytes).ok()?;
    trim_and_validate_node_name(s).ok().map(str::to_string)
}

/// Responder tag: bound to both one-time numbers and both ids from TLS.
pub fn responder_tag(
    handshake_key: &[u8; 32],
    initiator_nonce: &[u8; 32],
    responder_nonce: &[u8; 32],
    initiator: &PeerId,
    responder: &PeerId,
) -> blake3::Hash {
    let mut tag_input = Vec::with_capacity(27 + 32 + 32 + 32 + 32);
    tag_input.extend_from_slice(b"meshtown auth responder v1");
    tag_input.extend_from_slice(initiator_nonce);
    tag_input.extend_from_slice(responder_nonce);
    tag_input.extend_from_slice(initiator.as_bytes());
    tag_input.extend_from_slice(responder.as_bytes());
    blake3::keyed_hash(handshake_key, &tag_input)
}

/// Initiator tag: the same, but with a different role label.
pub fn initiator_tag(
    handshake_key: &[u8; 32],
    initiator_nonce: &[u8; 32],
    responder_nonce: &[u8; 32],
    initiator: &PeerId,
    responder: &PeerId,
) -> blake3::Hash {
    let mut tag_input = Vec::with_capacity(27 + 32 + 32 + 32 + 32);
    tag_input.extend_from_slice(b"meshtown auth initiator v1");
    tag_input.extend_from_slice(initiator_nonce);
    tag_input.extend_from_slice(responder_nonce);
    tag_input.extend_from_slice(initiator.as_bytes());
    tag_input.extend_from_slice(responder.as_bytes());
    blake3::keyed_hash(handshake_key, &tag_input)
}

/// Waits for the list to be ready, then refuses a peer the list does not let in (code 403).
async fn admit_peer(conn: &Arc<dyn PeerConn>, ctx: &MemberContext) -> Result<()> {
    let access = ctx.members.access();
    access.ready().await;
    let Err(deny) = access.check(conn.peer()) else {
        return Ok(());
    };
    access.note_refusal(conn.peer(), Side::Me, deny);
    conn.close(ACCESS_CLOSE_CODE, deny.wire_reason());
    bail!("peer refused by the membership list: {}", deny.code())
}

/// Context of a handshake error: the responder closed the connection with this refusal.
#[derive(Debug, Clone, Copy)]
pub struct RefusedByPeer(pub Deny);

impl std::fmt::Display for RefusedByPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "peer refused us: {}", self.0.code())
    }
}

/// The responder did not answer: if it closed with a refusal, remember it (the close frame may arrive a moment later).
async fn learn_refusal(conn: &Arc<dyn PeerConn>, ctx: &MemberContext) -> Option<Deny> {
    let _ = timeout(Duration::from_secs(1), conn.closed()).await;
    let reason = Deny::from_wire(&conn.close_reason());
    let access = ctx.members.access();
    reason
        .iter()
        .for_each(|deny| access.note_refusal(conn.peer(), Side::Them, *deny));
    reason
}

/// Reads the responder's nonce and tag; a failed read may mean the responder refused us.
async fn read_reply(
    conn: &Arc<dyn PeerConn>,
    ctx: &MemberContext,
    recv: &mut (impl AsyncReadExt + Unpin),
) -> Result<[u8; 64]> {
    let mut reply = [0u8; 64];
    let read = recv.read_exact(&mut reply).await;
    let Err(err) = read else { return Ok(reply) };
    let refusal = learn_refusal(conn, ctx).await;
    Err(with_refusal(err.into(), refusal))
}

/// A failed exchange: if the responder's refusal is not attached yet, learns it from the close frame.
async fn typed_refusal(
    err: anyhow::Error,
    conn: &Arc<dyn PeerConn>,
    ctx: &MemberContext,
) -> anyhow::Error {
    match err.downcast_ref::<RefusedByPeer>() {
        Some(_) => err,
        None => with_refusal(err, learn_refusal(conn, ctx).await),
    }
}

/// Attaches the responder's refusal, if any, to the error as a typed context.
fn with_refusal(err: anyhow::Error, refusal: Option<Deny>) -> anyhow::Error {
    match refusal {
        Some(deny) => err.context(RefusedByPeer(deny)),
        None => err,
    }
}

/// The initiator's and the responder's nonces of one handshake.
type Nonces<'a> = (&'a [u8; 32], &'a [u8; 32]);

/// The tag the responder must send when it is `ctx.own_id` talking to `peer` (the initiator side check).
fn responder_tag_for(ctx: &MemberContext, nonces: Nonces, peer: &PeerId) -> blake3::Hash {
    responder_tag(&ctx.handshake_key, nonces.0, nonces.1, &ctx.own_id, peer)
}

/// The tag the initiator `peer` must send to the responder `ctx.own_id`.
fn initiator_tag_for(ctx: &MemberContext, nonces: Nonces, peer: &PeerId) -> blake3::Hash {
    initiator_tag(&ctx.handshake_key, nonces.0, nonces.1, peer, &ctx.own_id)
}

/// The responder's reply: its nonce followed by its tag.
fn reply_frame(ctx: &MemberContext, nonces: Nonces, peer: &PeerId) -> [u8; 64] {
    let mut reply = [0u8; 64];
    reply[..32].copy_from_slice(nonces.1);
    let tag = responder_tag(&ctx.handshake_key, nonces.0, nonces.1, peer, &ctx.own_id);
    reply[32..].copy_from_slice(tag.as_bytes());
    reply
}

/// The responder's closing frame: the ack byte followed by its own name.
fn ack_frame(ctx: &MemberContext) -> Vec<u8> {
    let own_name = ctx.own_name_tx.borrow().clone();
    let mut frame = vec![1u8];
    frame.extend_from_slice(&name_frame(&own_name));
    frame
}

/// The initiator's closing frame: its tag followed by its own name.
fn initiator_frame(ctx: &MemberContext, nonces: Nonces, peer: &PeerId) -> Vec<u8> {
    let own_name = ctx.own_name_tx.borrow().clone();
    let tag = initiator_tag(&ctx.handshake_key, nonces.0, nonces.1, &ctx.own_id, peer);
    let mut frame = Vec::with_capacity(32 + 258);
    frame.extend_from_slice(tag.as_bytes());
    frame.extend_from_slice(&name_frame(&own_name));
    frame
}

/// Starts the handshake on an already open connection. The initiator is always the one who dials.
pub async fn initiate(conn: &Arc<dyn PeerConn>, ctx: &MemberContext) -> Result<()> {
    let peer = conn.peer();
    let talk = async {
        let StreamPair { mut send, mut recv } = conn.open().await.map_err(NetError::into_anyhow)?;
        let mut initiator_nonce = [0u8; 32];
        fill_random(&mut initiator_nonce)?;
        send.write_all(&initiator_nonce).await?;
        let reply = read_reply(conn, ctx, &mut recv).await?;
        let responder_nonce: [u8; 32] = reply[..32].try_into().unwrap();
        let received_tag = blake3::Hash::from_bytes(reply[32..].try_into().unwrap());
        let nonces = (&initiator_nonce, &responder_nonce);
        if received_tag != responder_tag_for(ctx, nonces, &peer) {
            conn.close(401, b"bad secret");
            bail!("peer's tag did not match — the key of another network {NET_NAME}");
        }
        let tag_and_name_frame = initiator_frame(ctx, (&initiator_nonce, &responder_nonce), &peer);
        send.write_all(&tag_and_name_frame).await?;
        send.shutdown().await?;
        let mut ack_byte = [0u8; 1];
        recv.read_exact(&mut ack_byte).await?;
        ensure!(
            ack_byte[0] == 1,
            "peer answered {} instead of 1",
            ack_byte[0]
        );
        let tail = read_to_end_limited(&mut recv, MAX_TAIL_BYTES)
            .await
            .unwrap_or_default();
        anyhow::Ok(tail)
    };
    let exchange = async {
        admit_peer(conn, ctx).await?;
        match talk.await {
            Err(e) => Err(typed_refusal(e, conn, ctx).await),
            ok => ok,
        }
    };
    let tail = match timeout(HANDSHAKE_TIMEOUT, exchange).await {
        Err(_) => {
            conn.close(408, b"timeout");
            bail!("handshake did not finish within 10 s")
        }
        Ok(r) => r?,
    };
    announce_up(conn, ctx, &tail);
    Ok(())
}

/// The initiator's handshake is done: remembers the name from the tail and announces the member.
fn announce_up(conn: &Arc<dyn PeerConn>, ctx: &MemberContext, tail: &[u8]) {
    let peer = conn.peer();
    if let Some(name) = parse_name_tail(tail) {
        ctx.members.set_name(peer, name);
    }
    if ctx.members.add_verified_conn(peer) {
        let _ = ctx.events.send(MemberEvent::Up {
            peer,
            conn: conn.clone(),
        });
    }
}

/// Answers a handshake started by the peer (the mirror of `initiate`).
pub async fn respond(conn: &Arc<dyn PeerConn>, ctx: &MemberContext) -> Result<()> {
    let peer = conn.peer();
    let exchange = async {
        admit_peer(conn, ctx).await?;
        let StreamPair { mut send, mut recv } =
            conn.accept().await.map_err(NetError::into_anyhow)?;
        let mut initiator_nonce = [0u8; 32];
        recv.read_exact(&mut initiator_nonce).await?;
        let mut responder_nonce = [0u8; 32];
        fill_random(&mut responder_nonce)?;
        let nonces = (&initiator_nonce, &responder_nonce);
        let reply = reply_frame(ctx, nonces, &peer);
        send.write_all(&reply).await?;
        let mut tag = [0u8; 32];
        recv.read_exact(&mut tag).await?;
        let received_tag = blake3::Hash::from_bytes(tag);
        if received_tag != initiator_tag_for(ctx, nonces, &peer) {
            conn.close(401, b"bad secret");
            bail!("initiator's tag did not match — the key of another network {NET_NAME}");
        }
        let tail = read_to_end_limited(&mut recv, MAX_TAIL_BYTES)
            .await
            .unwrap_or_default();
        anyhow::Ok((send, tail))
    };
    let (mut send, tail) = match timeout(HANDSHAKE_TIMEOUT, exchange).await {
        Err(_) => {
            conn.close(408, b"timeout");
            bail!("handshake did not finish within 10 s")
        }
        Ok(r) => r?,
    };
    if let Some(name) = parse_name_tail(&tail) {
        ctx.members.set_name(peer, name);
    }
    if ctx.members.add_verified_conn(peer) {
        let _ = ctx.events.send(MemberEvent::Up {
            peer,
            conn: conn.clone(),
        });
    }
    // a write error is not counted as a failure here — membership::hold_conn_until_closed will see the connection close
    let ack_and_name_frame = ack_frame(ctx);
    let _ = send.write_all(&ack_and_name_frame).await;
    let _ = send.shutdown().await;
    Ok(())
}

/// AUTH service: answers the handshake and holds the member until the connection closes.
/// It returns no errors outward — on refusal it writes to the log itself (`say` and WARN).
pub(crate) struct AuthService {
    ctx: MemberContext,
}

/// Handshake handler for `NetStack`.
pub fn handshake_service(ctx: MemberContext) -> Arc<dyn ServiceHandler> {
    Arc::new(AuthService { ctx })
}

impl ServiceHandler for AuthService {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let ctx = self.ctx.clone();
        Box::pin(async move {
            let peer = conn.peer();
            match respond(&conn, &ctx).await {
                Ok(()) => {
                    say(
                        "auth",
                        format!("incoming from {}: success", short_peer_id(&peer)),
                    );
                    membership::hold_conn_until_closed(conn, peer, ctx).await;
                }
                Err(e) => {
                    say(
                        "auth",
                        format!("incoming from {}: refused — {e:#}", short_peer_id(&peer)),
                    );
                    tracing::warn!("Handling incoming connection ended with error: {e}");
                }
            }
        })
    }
}

#[cfg(test)]
#[path = "handshake_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "handshake_peer_tests.rs"]
mod peer_tests;
