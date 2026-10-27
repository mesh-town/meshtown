//! A stub of the invite server for tests: accepts a connection on `INVITE`, reads one request
//! (`REQUEST_LEN` bytes: the kind and the secret, the same for both frame kinds) and answers with a
//! frame taken from a script.
//!
//! `Responder::start` answers every request with one frame; `Responder::scripted` answers the first
//! request with the first frame, the second with the second and so on (the last frame repeats).
//! The node seed passed to `start`/`scripted` must match the key of the transport passed to it.

use crate::invite::redeem::{Progress, RedeemOpts};
use crate::invite::wire::{encode_invite_reply, RedeemCode, REPLY_LEN, REQUEST_LEN};
use crate::invite::{InviteHead, InviteSecret};
use crate::members::network_key::{managed_key, NetworkKey};
use crate::transport::net::facade::test_support::secret;
use crate::transport::net::{
    BoxFuture, NetStack, NetStackBuilder, NodeSecret, PeerAddr, PeerConn, ServiceHandler,
    StreamPair, Transport, INVITE,
};
use crate::transport::net_mainline::unix_now;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A server stub of the invite protocol: reads a request and answers from its script.
pub(crate) struct Responder {
    /// Keeps the stack alive: without it the handler would stop accepting connections.
    _stack: NetStack,
    node: Arc<dyn Transport>,
    seen: Seen,
}

type Seen = Arc<Mutex<Vec<Arc<dyn PeerConn>>>>;

impl Responder {
    /// A stub on the transport `node` that answers every request with `reply`.
    pub(crate) fn start(node: Arc<dyn Transport>, seed: u8, reply: [u8; REPLY_LEN]) -> Responder {
        Responder::scripted(node, seed, vec![reply])
    }

    /// A stub on the transport `node` that answers the n-th request with `replies[n]`; once the script
    /// runs out, the last frame is repeated.
    pub(crate) fn scripted(
        node: Arc<dyn Transport>,
        seed: u8,
        replies: Vec<[u8; REPLY_LEN]>,
    ) -> Responder {
        assert!(!replies.is_empty(), "a script needs at least one reply");
        let seen = Seen::default();
        let handler = Arc::new(Answering {
            replies,
            next: AtomicUsize::new(0),
            seen: seen.clone(),
        });
        let built = NetStackBuilder::new(secret(seed))
            .transport(node.clone())
            .serve(INVITE, handler)
            .build();
        Responder {
            _stack: built.expect("the stub stack was built"),
            node,
            seen,
        }
    }

    pub(crate) fn addr(&self) -> PeerAddr {
        self.node.local_addr()
    }

    /// Connections that reached the handler.
    pub(crate) fn conns(&self) -> Vec<Arc<dyn PeerConn>> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// How many requests reached the handler.
    pub(crate) fn requests(&self) -> usize {
        self.conns().len()
    }
}

struct Answering {
    replies: Vec<[u8; REPLY_LEN]>,
    next: AtomicUsize,
    seen: Seen,
}

impl ServiceHandler for Answering {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(conn.clone());
        let n = self.next.fetch_add(1, Ordering::SeqCst);
        let reply = self.replies[n.min(self.replies.len() - 1)];
        Box::pin(async move {
            let _ = answer(conn.as_ref(), reply).await;
            conn.closed().await;
        })
    }
}

async fn answer(conn: &dyn PeerConn, reply: [u8; REPLY_LEN]) -> io::Result<()> {
    let accepted = conn.accept().await;
    let StreamPair { mut send, mut recv } =
        accepted.map_err(|e| io::Error::other(e.to_string()))?;
    let mut request = [0u8; REQUEST_LEN];
    recv.read_exact(&mut request).await?;
    send.write_all(&reply).await?;
    send.shutdown().await
}

/// The base of the key of the network the invite was issued from: what a member answers with.
pub(crate) fn own_key() -> NetworkKey {
    NetworkKey::from_bytes([1; 32])
}

/// The head of an invite of the network made from `own_key()`, just issued.
pub(crate) fn head() -> InviteHead {
    let root = NodeSecret::from_bytes(&[7; 32]).peer_id();
    InviteHead {
        secret: InviteSecret::from_bytes([9; 32]),
        network_id: managed_key(&own_key().to_bytes(), &root)
            .subkeys()
            .network_id,
        label: None,
        made_unix: unix_now(),
        root,
    }
}

/// Pauses of 30 s, the deadline in a minute, one "unknown" round is the outcome: a deciding answer must
/// exit without pauses, otherwise `within` in the test will time out.
pub(crate) fn opts() -> RedeemOpts {
    RedeemOpts {
        first_pause: Duration::from_secs(30),
        max_pause: Duration::from_secs(30),
        deadline_unix: unix_now() + 60,
        unknown_rounds: 1,
    }
}

/// The reply "network key".
pub(crate) fn key_frame() -> [u8; REPLY_LEN] {
    encode_invite_reply(RedeemCode::Ok, Some(&own_key()))
}

/// A reply with a refusal code and no key.
pub(crate) fn refusal_frame(code: RedeemCode) -> [u8; REPLY_LEN] {
    encode_invite_reply(code, None)
}

pub(crate) fn ignore(_: Progress) {}

/// The permanent key of the joining node: it goes into the owner frame.
pub(crate) fn owner() -> NodeSecret {
    secret(5)
}
