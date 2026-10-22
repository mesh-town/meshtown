//! A raw handshake peer on the `mem` network: the protocol bytes are written by hand, without a node and a document.
//!
//! What it does: provides a "raw" initiator (calls a node, gives its name, returns the name from the tail of
//! the reply) and a "raw" responder (accepts calls, puts the initiator's tail into a channel). The labels and the name
//! frame are the same `handshake` functions as in a real node. Included via `#[path]`.
#![allow(dead_code)] // each test crate takes its own subset

use std::sync::Arc;

use anyhow::Result;
use meshtown_core::members::handshake::{
    initiator_tag, name_frame, parse_name_tail, responder_tag,
};
use meshtown_core::members::network_key::{fill_random, NetworkKey};
use meshtown_core::transport::net::mem::process_net;
use meshtown_core::transport::net::{
    read_to_end_limited, NodeSecret, PeerConn, PeerId, SplitTransport, StreamPair, Transport, AUTH,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

/// A raw `mem` transport with a fresh secret in the process-wide network: its id and the transport itself.
fn raw_transport() -> (PeerId, Arc<SplitTransport>) {
    let secret = NodeSecret::generate();
    (secret.peer_id(), process_net().transport(&secret))
}

/// A raw initiator of the current protocol version: calls `target`, gives its name, returns the name
/// from the tail of `target`'s reply.
pub(crate) async fn raw_initiate_to(target: PeerId, key: &NetworkKey) -> Option<String> {
    let (me, transport) = raw_transport();
    let hk = key.subkeys().handshake_key;
    let addr = process_net()
        .addr_of(target)
        .expect("the node is not in the mem network");
    let conn = transport.connect(&addr, AUTH).await.unwrap();
    let StreamPair { mut send, mut recv } = conn.open().await.unwrap();
    let mut nonce_a = [0u8; 32];
    fill_random(&mut nonce_a).unwrap();
    send.write_all(&nonce_a).await.unwrap();
    let mut reply = [0u8; 64];
    recv.read_exact(&mut reply).await.unwrap();
    let nonce_b: [u8; 32] = reply[..32].try_into().unwrap();
    let tag = initiator_tag(&hk, &nonce_a, &nonce_b, &me, &target);
    let mut out = tag.as_bytes().to_vec();
    out.extend_from_slice(&name_frame("raw-initiator"));
    send.write_all(&out).await.unwrap();
    send.shutdown().await.unwrap();
    let mut ok = [0u8; 1];
    recv.read_exact(&mut ok).await.unwrap();
    assert_eq!(ok[0], 1, "the responder did not accept the handshake");
    let tail = read_to_end_limited(&mut recv, 1024).await.unwrap();
    conn.close(0, b"done");
    transport.close().await;
    parse_name_tail(&tail)
}

/// A raw responder of the current protocol version: puts the initiator's tail (the name frame) into a channel.
pub(crate) struct RawResponder {
    /// Its id in the `mem` network: the node dials the responder by it.
    pub(crate) id: PeerId,
    transport: Arc<SplitTransport>,
}

impl RawResponder {
    /// Brings up the responder: itself and the tail receiver. Stop it at the end of the test with `stop`.
    pub(crate) fn spawn(key: &NetworkKey) -> (RawResponder, mpsc::UnboundedReceiver<Vec<u8>>) {
        let (id, transport) = raw_transport();
        transport.serve(&[AUTH]);
        let (tails, rx) = mpsc::unbounded_channel();
        let (hk, t) = (key.subkeys().handshake_key, transport.clone());
        tokio::spawn(async move {
            while let Some((_, conn)) = t.accept().await {
                let tails = tails.clone();
                tokio::spawn(async move {
                    let _ = respond(&hk, id, &*conn, &tails).await;
                    conn.closed().await;
                });
            }
        });
        (RawResponder { id, transport }, rx)
    }

    pub(crate) async fn stop(self) {
        self.transport.close().await;
    }
}

async fn respond(
    key: &[u8; 32],
    me: PeerId,
    conn: &dyn PeerConn,
    tails: &mpsc::UnboundedSender<Vec<u8>>,
) -> Result<()> {
    let peer = conn.peer();
    let StreamPair { mut send, mut recv } = conn.accept().await?;
    let mut nonce_a = [0u8; 32];
    recv.read_exact(&mut nonce_a).await?;
    let mut nonce_b = [0u8; 32];
    fill_random(&mut nonce_b)?;
    let mut reply = [0u8; 64];
    reply[..32].copy_from_slice(&nonce_b);
    let tag = responder_tag(key, &nonce_a, &nonce_b, &peer, &me);
    reply[32..].copy_from_slice(tag.as_bytes());
    send.write_all(&reply).await?;
    let mut got_tag = [0u8; 32];
    recv.read_exact(&mut got_tag).await?;
    let _ = tails.send(read_to_end_limited(&mut recv, 1024).await?);
    let mut out = vec![1u8];
    out.extend_from_slice(&name_frame("raw-responder"));
    send.write_all(&out).await?;
    send.shutdown().await?;
    Ok(())
}
