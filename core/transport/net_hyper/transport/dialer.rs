//! Dials DHT connections of the node and checks the other side's key.

use super::held::Held;
use super::hints::HintBook;
use super::race::first_ok;
use super::{dht_error_to_net_error, CONNECT_LIMIT};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net_hyper::conn::StreamDialer;
use crate::transport::net_hyper::keys::peer_of;
use crate::transport::net_hyper::link::PathFacts;
use std::net::SocketAddrV4;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use townjump_dht::{ConnectOpts, Dht, DhtConnection, Duplex};
use townjump_dht_peeroxide::PeeroxideDht;

pub(super) struct NodeDialer {
    pub(super) node: Arc<PeeroxideDht>,
    /// Simultaneous dials: the incoming queue of the townjump server has 16 slots, a larger burst drops connections.
    pub(super) slots: Arc<Semaphore>,
    /// Addresses for direct dials, and those that worked.
    pub(super) hints: HintBook,
}

/// A connection with the address that was dialed directly (`None` for a dial by key).
struct Reached {
    conn: DhtConnection,
    via: Option<SocketAddrV4>,
}

impl Reached {
    fn checked(self, to: PeerId) -> NetResult<Self> {
        let got = peer_of(&self.conn.remote_public_key)?;
        match got == to {
            true => Ok(self),
            false => Err(NetError::WrongPeer { want: to, got }),
        }
    }
}

impl NodeDialer {
    /// One slot for the whole dial: the key lookup and the direct attempts race, the slot goes to the winner.
    pub(super) async fn dial_with_path(
        &self,
        to: PeerId,
    ) -> NetResult<(Box<dyn Duplex>, PathFacts)> {
        timeout(CONNECT_LIMIT, self.race(to))
            .await
            .map_err(|_| NetError::Timeout)?
    }

    async fn race(&self, to: PeerId) -> NetResult<(Box<dyn Duplex>, PathFacts)> {
        let slot = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| NetError::Closed)?;
        let direct = self
            .hints
            .targets(to)
            .into_iter()
            .map(|at| -> BoxFuture<'_, NetResult<Reached>> { Box::pin(self.direct(to, at)) })
            .collect();
        let key: BoxFuture<'_, NetResult<Reached>> = Box::pin(self.by_key(to));
        let reached = first_ok(key, direct).await?;
        let facts = PathFacts::dialed(&reached.conn, reached.via);
        Ok((Box::new(Held::new(reached.conn.stream, slot)), facts))
    }

    async fn by_key(&self, to: PeerId) -> NetResult<Reached> {
        // reusable_socket: after the first hole punch the fork remembers the route, the next connect goes without it.
        let opts = ConnectOpts {
            reusable_socket: true,
            ..Default::default()
        };
        let conn = self
            .node
            .connect(*to.as_bytes(), opts)
            .await
            .map_err(dht_error_to_net_error)?;
        Reached { conn, via: None }.checked(to)
    }

    async fn direct(&self, to: PeerId, at: SocketAddrV4) -> NetResult<Reached> {
        let dialed = self.node.connect_direct(*to.as_bytes(), at).await;
        let reached = dialed.map_err(dht_error_to_net_error).and_then(|conn| {
            Reached {
                conn,
                via: Some(at),
            }
            .checked(to)
        });
        match &reached {
            Ok(_) => self.hints.prove(to, at),
            Err(_) => self.hints.disprove(to, at),
        }
        reached
    }
}

impl StreamDialer for NodeDialer {
    fn dial(&self, to: PeerId) -> BoxFuture<'_, NetResult<Box<dyn Duplex>>> {
        Box::pin(async move { Ok(self.dial_with_path(to).await?.0) })
    }
}
