//! Accepting `SYNC` service connections: the accepting side's handshake and handing the link to the topic.

use super::admit::accept_handshake;
use super::backend::FloodShared;
use super::topic::TopicCmd;
use super::wire::CLOSE_REJECTED;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::stream::{BoxFuture, StreamPair};
use crate::transport::net::transport::PeerConn;
use crate::transport::net::ServiceHandler;
use std::sync::Arc;
use tokio::time::timeout;

pub struct FloodService {
    pub(super) shared: Arc<FloodShared>,
}

impl ServiceHandler for FloodService {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let shared = self.shared.clone();
        Box::pin(async move { serve(shared, conn).await })
    }
}

async fn serve(shared: Arc<FloodShared>, conn: Arc<dyn PeerConn>) {
    let Ok((pair, topic)) = admit(&shared, &conn).await else {
        return conn.close(CLOSE_REJECTED, b"rejected");
    };
    let Some(cmd) = shared.topic(&topic) else {
        return conn.close(CLOSE_REJECTED, b"no topic");
    };
    let peer = conn.peer();
    let linked = TopicCmd::Linked {
        peer,
        dialer: peer,
        conn: conn.clone(),
        pair,
    };
    if cmd.send(linked).await.is_err() {
        conn.close(CLOSE_REJECTED, b"topic gone");
    }
}

/// The first stream and `Hello` within the handshake timeout; a refusal, timeout or break is `Err`.
async fn admit(
    shared: &FloodShared,
    conn: &Arc<dyn PeerConn>,
) -> NetResult<(StreamPair, [u8; 32])> {
    let handshake = async {
        let mut pair = conn.accept().await?;
        let known = |t: &[u8; 32]| shared.topic(t).is_some();
        let topic = accept_handshake(
            &mut pair,
            &shared.admission_key,
            shared.me,
            conn.peer(),
            known,
        )
        .await?;
        Ok((pair, topic))
    };
    timeout(shared.tuning.handshake, handshake)
        .await
        .unwrap_or(Err(NetError::Timeout))
}
