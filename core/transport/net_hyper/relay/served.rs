//! What a running relay can offer to the network right now, judged by its own reachability reading.

use crate::transport::net_hyper::relay::RelayAdvert;
use std::future::Future;
use std::net::SocketAddrV4;
use townjump_dht::DhtError;
use townjump_dht_peeroxide::{NatKind, PeeroxideRelay, RelayReachability};

/// What the relay can be announced as: not yet known, a relay to serve with a record, or unfit to serve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Offer {
    /// The reading is not conclusive yet.
    Wait,
    /// Announce this record: with an address for an open relay, key alone behind a consistent NAT.
    Serve(RelayAdvert),
    /// The NAT maps every peer to another port: nobody can pair through this relay.
    Unfit,
}

/// The offer for a relay with `key`, the NAT kind it measured and its public address, when known.
pub(crate) fn offer_of(key: [u8; 32], kind: NatKind, address: Option<SocketAddrV4>) -> Offer {
    match kind {
        NatKind::Open => address.map_or(Offer::Wait, |a| serve(key, Some(a))),
        NatKind::Consistent => serve(key, None),
        NatKind::Random => Offer::Unfit,
        NatKind::Unknown => Offer::Wait,
    }
}

/// The offer for a reachability reading; a failed reading is not conclusive.
pub(crate) fn offer_of_reading(
    key: [u8; 32],
    reading: Result<RelayReachability, DhtError>,
) -> Offer {
    match reading {
        Ok(r) => offer_of(key, r.firewall, r.public_address),
        Err(_) => Offer::Wait,
    }
}

fn serve(key: [u8; 32], address: Option<SocketAddrV4>) -> Offer {
    Offer::Serve(RelayAdvert { key, address })
}

/// A running relay node as the host sees it.
pub(crate) trait ServedRelay: Send + Sync + 'static {
    /// The key of the relay node.
    fn key(&self) -> [u8; 32];
    /// What the relay can be announced as now.
    fn offer(&self) -> impl Future<Output = Offer> + Send;
    /// Closes the relay node.
    fn close(self) -> impl Future<Output = ()> + Send;
}

impl ServedRelay for PeeroxideRelay {
    fn key(&self) -> [u8; 32] {
        self.public_key()
    }

    async fn offer(&self) -> Offer {
        let reading = self.reachability().await;
        if let Ok(r) = &reading {
            tracing::debug!(
                "relay: nat {:?}, address {:?}",
                r.firewall,
                r.public_address
            );
        }
        offer_of_reading(self.key(), reading)
    }

    async fn close(self) {
        PeeroxideRelay::close(self).await;
    }
}

#[cfg(test)]
#[path = "served_tests.rs"]
mod tests;
