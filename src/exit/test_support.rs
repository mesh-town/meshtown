//! Shared fixtures of exit tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::net::{BoxFuture, PeerConn, ServiceHandler};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A scripted exit: reads the whole request frame, writes `reply` and closes the stream.
/// `hits` counts the requests it has received.
pub(crate) struct Scripted {
    reply: &'static [u8],
    pub hits: Arc<AtomicUsize>,
}

impl Scripted {
    pub fn new(reply: &'static [u8]) -> Scripted {
        Scripted { reply, hits: Arc::default() }
    }
}

impl ServiceHandler for Scripted {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let reply = self.reply;
        let hits = self.hits.clone();
        Box::pin(async move {
            while let Ok(mut pair) = conn.accept().await {
                let mut head = [0u8; 2];
                if pair.recv.read_exact(&mut head).await.is_err() {
                    continue;
                }
                let mut tail = vec![0u8; usize::from(head[1]) + 2];
                let _ = pair.recv.read_exact(&mut tail).await;
                hits.fetch_add(1, Ordering::SeqCst);
                let _ = pair.send.write_all(reply).await;
                let _ = pair.send.shutdown().await;
            }
        })
    }
}
