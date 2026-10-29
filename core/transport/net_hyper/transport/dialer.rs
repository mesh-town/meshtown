//! Dials DHT connections of the node and checks the other side's key.

use super::held::Held;
use super::hints::HintBook;
use super::phases::{direct_then_relay, Phase};
use super::race::first_ok;
use super::relay::RelayDial;
use super::{dht_error_to_net_error, CONNECT_LIMIT, DIRECT_LIMIT, RELAY_LIMIT};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net_hyper::conn::StreamDialer;
use crate::transport::net_hyper::keys::peer_of;
use crate::transport::net_hyper::link::{relay_label, PathFacts};
use crate::transport::net_hyper::relay::{RelayCandidate, RelayPick};
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;
use townjump_dht::{ConnectOpts, Dht, DhtConnection, Duplex, RelayAddress, RelayThrough};
use townjump_dht_peeroxide::PeeroxideDht;

pub(super) struct NodeDialer {
    pub(super) node: Arc<PeeroxideDht>,
    /// Simultaneous dials: the incoming queue of the townjump server has 16 slots, a larger burst drops connections.
    pub(super) slots: Arc<Semaphore>,
    /// Addresses for direct dials, and those that worked.
    pub(super) hints: HintBook,
    /// Relay candidates for dials and the memory of which relay worked.
    pub(super) relay: RelayDial,
}

/// Options of a dial that pairs through the relay of `cand`. With `vetoed` (tests only) the
/// hole punch is refused, so the connection can only come through the relay.
pub(super) fn relay_connect_opts(cand: &RelayCandidate, vetoed: bool) -> ConnectOpts {
    let relay_addresses = cand
        .address
        .map(|addr| RelayAddress {
            public_key: cand.relay,
            addr,
        })
        .into_iter()
        .collect();
    let opts = ConnectOpts {
        reusable_socket: true,
        relay_through: Some(RelayThrough::Key(cand.relay)),
        relay_addresses,
        ..Default::default()
    };
    veto_punch(opts, vetoed)
}

#[cfg(test)]
fn veto_punch(opts: ConnectOpts, vetoed: bool) -> ConnectOpts {
    if !vetoed {
        return opts;
    }
    ConnectOpts {
        holepunch: Some(Arc::new(|_| false)),
        local_connection: false,
        ..opts
    }
}

#[cfg(not(test))]
fn veto_punch(opts: ConnectOpts, _vetoed: bool) -> ConnectOpts {
    opts
}

/// Packs a reached connection with its slot; `through` is the member whose relay carried it.
fn held(
    reached: Reached,
    slot: OwnedSemaphorePermit,
    through: Option<PeerId>,
) -> (Box<dyn Duplex>, PathFacts) {
    let mut facts = PathFacts::dialed(&reached.conn, reached.via);
    facts.relay = relay_label(through, reached.conn.is_relayed());
    (Box::new(Held::new(reached.conn.stream, slot)), facts)
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
    /// One slot for the whole dial. Without a relay candidate the key lookup and the direct attempts race
    /// under `CONNECT_LIMIT`; with one, the direct phase goes first and the relay phase follows.
    pub(super) async fn dial_with_path(
        &self,
        to: PeerId,
    ) -> NetResult<(Box<dyn Duplex>, PathFacts)> {
        let list = self.relay.candidates(to).await;
        match self.relay.book.pick(to, &list, Instant::now()) {
            None => timeout(CONNECT_LIMIT, self.race(to))
                .await
                .map_err(|_| NetError::Timeout)?,
            Some(pick) => self.dial_phased(to, pick).await,
        }
    }

    async fn slot(&self) -> NetResult<OwnedSemaphorePermit> {
        let slots = self.slots.clone();
        slots.acquire_owned().await.map_err(|_| NetError::Closed)
    }

    async fn race(&self, to: PeerId) -> NetResult<(Box<dyn Duplex>, PathFacts)> {
        let slot = self.slot().await?;
        let reached = self.first_phase(to).await?;
        Ok(held(reached, slot, None))
    }

    /// The key lookup and the direct attempts at the known addresses; the first success wins.
    async fn first_phase(&self, to: PeerId) -> NetResult<Reached> {
        if self.relay.direct_fails() {
            return Err(NetError::NoRoute);
        }
        let direct = self
            .hints
            .targets(to)
            .into_iter()
            .map(|at| -> BoxFuture<'_, NetResult<Reached>> { Box::pin(self.direct(to, at)) })
            .collect();
        let key: BoxFuture<'_, NetResult<Reached>> = Box::pin(self.by_key(to));
        first_ok(key, direct).await
    }

    async fn dial_phased(
        &self,
        to: PeerId,
        pick: RelayPick,
    ) -> NetResult<(Box<dyn Duplex>, PathFacts)> {
        let slot = self.slot().await?;
        let direct = pick.direct_first.then(|| {
            self.relay.count_direct_run();
            Box::pin(self.first_phase(to)) as BoxFuture<'_, NetResult<Reached>>
        });
        let relay: BoxFuture<'_, NetResult<Reached>> = Box::pin(self.relay_phase(to, pick.cand));
        let outcome = direct_then_relay(direct, DIRECT_LIMIT, Some(relay), RELAY_LIMIT).await;
        self.settle(to, pick.cand, &outcome);
        let (reached, phase) = outcome?;
        let through = (phase == Phase::Relay).then_some(pick.cand.member);
        Ok(held(reached, slot, through))
    }

    /// Notes the outcome in the book and the log.
    fn settle(&self, to: PeerId, cand: RelayCandidate, outcome: &NetResult<(Reached, Phase)>) {
        match outcome {
            Ok((_, Phase::Relay)) => {
                self.relay.book.succeeded(to, cand, Instant::now());
                tracing::debug!("connected to {to} via relay of {}", cand.member);
            }
            Err(NetError::Closed) | Ok((_, Phase::Direct)) => {}
            Err(e) => {
                self.relay.book.failed(to, cand);
                tracing::debug!("relay of {} did not reach {to}: {e}", cand.member);
            }
        }
    }

    async fn relay_phase(&self, to: PeerId, cand: RelayCandidate) -> NetResult<Reached> {
        if self.relay.relay_fails(&cand.relay) {
            return Err(NetError::NoRoute);
        }
        self.by_key_via(to, cand).await
    }

    async fn by_key_via(&self, to: PeerId, cand: RelayCandidate) -> NetResult<Reached> {
        let opts = relay_connect_opts(&cand, self.relay.direct_fails());
        let conn = self
            .node
            .connect(*to.as_bytes(), opts)
            .await
            .map_err(dht_error_to_net_error)?;
        Reached { conn, via: None }.checked(to)
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
