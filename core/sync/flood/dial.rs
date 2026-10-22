//! Dialing a peer from the topic actor: `connect`, stream and handshake under one timeout; the result goes to the actor.

use super::admit::dial_handshake;
use super::topic::{TopicCmd, TopicCtx};
use super::wire::CLOSE_REJECTED;
use crate::net::error::{NetError, NetResult};
use crate::net::id::PeerId;
use crate::net::stream::StreamPair;
use crate::net::transport::PeerConn;
use crate::net::{WeakStack, SYNC};
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Dialing a peer: the result goes to the actor (`Linked` or `DialFailed`).
pub(super) async fn dial_task(ctx: TopicCtx, peer: PeerId, tx: mpsc::Sender<TopicCmd>) {
    let cmd = match dial_once(&ctx, peer).await {
        Ok((conn, pair)) => TopicCmd::Linked {
            peer,
            dialer: ctx.me,
            conn,
            pair,
        },
        Err(err) => TopicCmd::DialFailed { peer, err },
    };
    let _ = tx.send(cmd).await;
}

/// The whole dial (`connect`, stream, handshake) fits in a single `Tuning::handshake` timeout.
async fn dial_once(ctx: &TopicCtx, peer: PeerId) -> NetResult<(Arc<dyn PeerConn>, StreamPair)> {
    let slot = OnceLock::new();
    let call = dial_steps(ctx, peer, &slot);
    let done = timeout(ctx.tuning.handshake, call).await;
    let done = done.unwrap_or(Err(NetError::Timeout));
    let close = |_: &NetError| {
        let conn = slot.get();
        conn.iter().for_each(|c| c.close(CLOSE_REJECTED, b"dial failed"));
    };
    done.inspect_err(close).map_err(|e| neighbor_closed(e, slot.get().is_some()))
}

/// `Closed` on a live link means the peer closed it, not the stack: it does not end topics.
fn neighbor_closed(err: NetError, linked: bool) -> NetError {
    if linked && matches!(err, NetError::Closed) {
        return NetError::Refused;
    }
    err
}

/// The dial steps; the ready link goes into `slot` so the timeout can close it.
async fn dial_steps(
    ctx: &TopicCtx,
    peer: PeerId,
    slot: &OnceLock<Arc<dyn PeerConn>>,
) -> NetResult<(Arc<dyn PeerConn>, StreamPair)> {
    let stack = ctx.stack.get().and_then(WeakStack::upgrade);
    let conn = stack.ok_or(NetError::Closed)?.connect(peer, SYNC).await?;
    let conn = slot.get_or_init(|| conn).clone();
    let mut pair = conn.open().await?;
    dial_handshake(&mut pair, &ctx.admission_key, ctx.me, peer, &ctx.topic).await?;
    Ok((conn, pair))
}
