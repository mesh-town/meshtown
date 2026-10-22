//! The `mem` connector: finding a slot by hint, handing out and accepting paths through the hub.
//! The path itself is in `path`, the network state is in `hub`.

use crate::transport::net::addr::{PeerAddr, RelayRef};
use crate::transport::net::connect::{Connector, PacketPath};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::link::NetInfo;
use crate::transport::net::mem::hub::{decode_hint, encode_hint, Hub};
use crate::transport::net::mem::path::MemPath;
use crate::transport::net::stream::BoxFuture;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct MemConnector {
    hub: Arc<Hub>,
    slot: u64,
    peer: PeerId,
    rx: Mutex<mpsc::Receiver<MemPath>>,
}

impl MemConnector {
    pub(super) fn new(
        hub: Arc<Hub>,
        slot: u64,
        peer: PeerId,
        rx: mpsc::Receiver<MemPath>,
    ) -> MemConnector {
        MemConnector {
            hub,
            slot,
            peer,
            rx: Mutex::new(rx),
        }
    }

    async fn dial_inner(&self, to: PeerAddr) -> NetResult<Box<dyn PacketPath>> {
        let (mine, theirs, queue, mut gone) = self.hub.open_link(self.slot, &to)?;
        tokio::select! {
            r = queue.send(theirs) => r.map_err(|_| NetError::NoRoute)?,
            _ = gone.wait_for(|g| *g) => return Err(NetError::NoRoute),
        }
        Ok(Box::new(mine))
    }

    async fn listen_inner(&self) -> Option<Box<dyn PacketPath>> {
        let mut rx = self.rx.lock().await;
        if !self.hub.is_live(self.slot) {
            return None;
        }
        let path = rx.recv().await?;
        if !self.hub.is_live(self.slot) {
            return None;
        }
        Some(Box::new(path))
    }
}

impl Connector for MemConnector {
    fn kind(&self) -> &'static str {
        "mem"
    }

    fn local_addr(&self) -> PeerAddr {
        let mut addr = PeerAddr::new(self.peer);
        addr.hints.push(encode_hint(self.hub.net_id, self.slot));
        addr
    }

    fn can_reach(&self, to: &PeerAddr) -> bool {
        let ours = |h| decode_hint(h).is_some_and(|(net, _)| net == self.hub.net_id);
        to.hints.iter().any(ours)
    }

    fn add_hints(&self, _addr: &PeerAddr) {}

    fn set_relays(&self, _relays: &[RelayRef]) {}

    fn dial(&self, to: &PeerAddr) -> BoxFuture<'_, NetResult<Box<dyn PacketPath>>> {
        Box::pin(self.dial_inner(to.clone()))
    }

    fn listen(&self) -> BoxFuture<'_, Option<Box<dyn PacketPath>>> {
        Box::pin(self.listen_inner())
    }

    fn net_info(&self) -> NetInfo {
        NetInfo::default()
    }

    fn close(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move { self.hub.close_slot(self.slot) })
    }
}

#[cfg(test)]
#[path = "connector_tests.rs"]
mod tests;
