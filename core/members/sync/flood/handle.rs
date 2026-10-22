//! The topic handle for the node: `Membership` on top of the actor's command channel.

use super::topic::TopicCmd;
use super::wire::MAX_BODY_BYTES;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::{Membership, SwarmEvent, WeakStack};
use bytes::Bytes;
use std::io::{Error, ErrorKind};
use std::sync::{Arc, OnceLock};
use tokio::sync::{broadcast, mpsc};

pub struct TopicHandle {
    pub(super) cmd: mpsc::Sender<TopicCmd>,
    pub(super) events: broadcast::Sender<SwarmEvent>,
    pub(super) stack: Arc<OnceLock<WeakStack>>,
}

impl TopicHandle {
    async fn send(&self, cmd: TopicCmd) -> NetResult<()> {
        self.cmd.send(cmd).await.map_err(|_| NetError::Closed)
    }
}

impl Membership for TopicHandle {
    fn join(&self, peers: &[PeerId]) -> BoxFuture<'_, NetResult<()>> {
        let peers = peers.to_vec();
        Box::pin(async move {
            if peers.is_empty() {
                return Ok(());
            }
            if self.stack.get().is_none() {
                return Err(NetError::Config(
                    "network stack is not attached to the sync".into(),
                ));
            }
            self.send(TopicCmd::Join(peers)).await
        })
    }

    fn broadcast(&self, msg: Bytes) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async move {
            if msg.len() > MAX_BODY_BYTES {
                let big = Error::new(ErrorKind::InvalidInput, "message larger than the limit");
                return Err(NetError::Io(big));
            }
            self.send(TopicCmd::Broadcast(msg)).await
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<SwarmEvent> {
        self.events.subscribe()
    }
}
