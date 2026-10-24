//! Dials DHT connections of the node and checks the other side's key.

use super::held::Held;
use super::{dht_error_to_net_error, CONNECT_LIMIT};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net_hyper::conn::StreamDialer;
use crate::transport::net_hyper::keys::peer_of;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use townjump_dht::{ConnectOpts, Dht, Duplex};
use townjump_dht_peeroxide::PeeroxideDht;

pub(super) struct NodeDialer {
    pub(super) node: Arc<PeeroxideDht>,
    /// Simultaneous dials: the incoming queue of the townjump server has 16 slots, a larger burst drops connections.
    pub(super) slots: Arc<Semaphore>,
}

impl NodeDialer {
    async fn dial_checked(&self, to: PeerId) -> NetResult<Box<dyn Duplex>> {
        let slot = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| NetError::Closed)?;
        // reusable_socket: after the first hole punch the fork remembers the route, the next connect goes without it.
        let conn = self
            .node
            .connect(
                *to.as_bytes(),
                ConnectOpts {
                    reusable_socket: true,
                    ..Default::default()
                },
            )
            .await
            .map_err(dht_error_to_net_error)?;
        let got = peer_of(&conn.remote_public_key)?;
        match got == to {
            true => Ok(Box::new(Held::new(conn.stream, slot))),
            false => Err(NetError::WrongPeer { want: to, got }),
        }
    }
}

impl StreamDialer for NodeDialer {
    fn dial(&self, to: PeerId) -> BoxFuture<'_, NetResult<Box<dyn Duplex>>> {
        Box::pin(async move {
            timeout(CONNECT_LIMIT, self.dial_checked(to))
                .await
                .map_err(|_| NetError::Timeout)?
        })
    }
}
