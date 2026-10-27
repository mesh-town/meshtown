//! Helpers for the relay tests: a relay node on a loopback cluster, a check that a relay was asked
//! to pair, and a fixed list of relay candidates.

use super::{RelayCandidate, RelaySource};
use crate::transport::net::addr::RelayRef;
use crate::transport::net::id::PeerId;
use crate::transport::net::link::PathKind;
use crate::transport::net_hyper::test_cluster::Cluster;
use std::sync::Arc;
use std::time::{Duration, Instant};
use townjump_dht_peeroxide::PeeroxideFactory;

/// A running relay node, named here so that tests outside the transport need no townjump import.
pub(crate) use townjump_dht_peeroxide::PeeroxideRelay;

/// How often `asked_within` looks at the relay counters.
const ASK_POLL: Duration = Duration::from_millis(100);

/// A relay node that is part of the cluster, with a random key.
pub(crate) async fn relay_on(cl: &Cluster) -> PeeroxideRelay {
    PeeroxideFactory {
        bootstrap: Some(cl.bootstrap()),
        ..Default::default()
    }
    .create_relay(None)
    .await
    .expect("relay came up")
}

/// Whether `relay` receives at least one pairing request within `limit`.
pub(crate) async fn asked_within(relay: &PeeroxideRelay, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if relay.stats().pairings_requested >= 1 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(ASK_POLL).await;
    }
}

/// A source that gives `list` for every target.
pub(crate) fn fixed_source(list: Vec<RelayCandidate>) -> RelaySource {
    Arc::new(move |_target: PeerId| {
        let list = list.clone();
        Box::pin(async move { list })
    })
}

/// The candidate "`member` runs `relay`".
pub(crate) fn candidate(member: PeerId, relay: &PeeroxideRelay) -> RelayCandidate {
    RelayCandidate {
        member,
        relay: relay.public_key(),
    }
}

/// Whether `path` is "relayed through the relay of `member`".
pub(crate) fn relayed_via(path: &PathKind, member: PeerId) -> bool {
    matches!(path, PathKind::Relayed(RelayRef::Peer(p)) if *p == member)
}
