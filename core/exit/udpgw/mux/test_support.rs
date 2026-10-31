//! Fixtures of the lane pool tests: a client stack connected to an exit stack on an in-memory network,
//! and an exit that answers the gateway request and keeps the stream open.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::exit::{PeerConnPool, EXIT_CODE_OK, EXIT_PROTOCOL_VERSION};
use crate::transport::net::facade::test_support::{pid, secret, Spy};
use crate::transport::net::mem::MemNet;
use crate::transport::net::service::EXIT;
use crate::transport::net::{
    BoxFuture, NetStack, NetStackBuilder, PeerConn, PeerId, ServiceHandler, StreamPair,
};

/// The reply of an exit that accepts the request.
pub(super) const OK: &[u8] = &[EXIT_PROTOCOL_VERSION, EXIT_CODE_OK];

/// A client stack and pool wired to an exit that runs `handler`; the exit stack is returned too so a test can stop it.
pub(super) async fn exit_pair_with_stack(
    handler: Arc<dyn ServiceHandler>,
) -> (NetStack, PeerConnPool, PeerId, NetStack) {
    let net = MemNet::new();
    let client = NetStackBuilder::new(secret(1))
        .transport(Spy::on(&net, &secret(1)).shared())
        .build()
        .expect("the client was built");
    let server = NetStackBuilder::new(secret(2))
        .transport(Spy::on(&net, &secret(2)).shared())
        .serve(EXIT, handler)
        .build()
        .expect("the exit was built");
    client.add_hints(server.invite_addrs()[0].clone());
    let pool = PeerConnPool::default().with_open_timeout(Duration::from_millis(500));
    (client, pool, pid(2), server)
}

/// Like `exit_pair_with_stack`; the exit stack lives until the end of the process.
pub(super) async fn exit_pair(
    handler: Arc<dyn ServiceHandler>,
) -> (NetStack, PeerConnPool, PeerId) {
    let (client, pool, peer, server) = exit_pair_with_stack(handler).await;
    std::mem::forget(server);
    (client, pool, peer)
}

/// An exit that reads the request, answers `reply` after `delay` and, for an OK reply, keeps the stream open.
/// With `then`, requests after the first get that reply instead.
pub(crate) struct HoldOpen {
    reply: &'static [u8],
    later: Mutex<Option<&'static [u8]>>,
    delay: Duration,
    /// Requests received.
    pub hits: Arc<AtomicUsize>,
    held: Arc<Mutex<Vec<StreamPair>>>,
}

impl HoldOpen {
    pub(crate) fn new(reply: &'static [u8], delay: Duration) -> HoldOpen {
        HoldOpen {
            reply,
            later: Mutex::new(None),
            delay,
            hits: Arc::default(),
            held: Arc::default(),
        }
    }

    /// Requests after the first are answered with `reply`.
    pub(crate) fn then(self, reply: &'static [u8]) -> HoldOpen {
        *self.later.lock().unwrap() = Some(reply);
        self
    }

    /// Closes every stream the exit holds.
    pub(crate) fn drop_all(&self) {
        self.held.lock().unwrap().clear();
    }
}

async fn serve_stream(
    mut pair: StreamPair,
    reply: &'static [u8],
    delay: Duration,
) -> Option<StreamPair> {
    tokio::time::sleep(delay).await;
    let _ = pair.send.write_all(reply).await;
    if reply != OK {
        let _ = pair.send.shutdown().await;
        return None;
    }
    Some(pair)
}

impl ServiceHandler for HoldOpen {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let (hits, held, delay) = (self.hits.clone(), self.held.clone(), self.delay);
        let first = self.reply;
        let later = self.later.lock().unwrap().unwrap_or(first);
        Box::pin(async move {
            while let Ok(mut pair) = conn.accept().await {
                let mut head = [0u8; 2];
                if pair.recv.read_exact(&mut head).await.is_err() {
                    continue;
                }
                let mut tail = vec![0u8; usize::from(head[1]) + 2];
                let _ = pair.recv.read_exact(&mut tail).await;
                let nth = hits.fetch_add(1, Ordering::SeqCst);
                let reply = if nth == 0 { first } else { later };
                let held = held.clone();
                crate::node::output::spawn(async move {
                    if let Some(pair) = serve_stream(pair, reply, delay).await {
                        held.lock().unwrap().push(pair);
                    }
                });
            }
        })
    }
}
