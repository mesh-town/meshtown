//! Helpers for the relay tests: a relay node on a loopback cluster, checks that a relay was asked
//! to pair and that it matched a pairing, and a fixed list of relay candidates.

use super::{RelayAdmit, RelayCandidate, RelaySource};
use crate::transport::net::addr::RelayRef;
use crate::transport::net::contract::secret;
use crate::transport::net::id::PeerId;
use crate::transport::net::link::PathKind;
use crate::transport::net_hyper::keys::relay_key_pair;
use crate::transport::net_hyper::relay::relay_opts;
use crate::transport::net_hyper::test_cluster::Cluster;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use townjump_dht_peeroxide::{PeeroxideFactory, RelayOpts};

/// A running relay node, named here so that tests outside the transport need no townjump import.
pub(crate) use townjump_dht_peeroxide::PeeroxideRelay;

/// How often `asked_within` and `matched_within` look at the relay counters.
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

/// An admission that lets every peer pair.
pub(crate) fn admit_all() -> RelayAdmit {
    Arc::new(|_peer: PeerId| true)
}

/// A relay node on the cluster with the permanent key of the node with seed `seed`, as a member runs it, except
/// that it is assumed open. It admits exactly the peers in `admitted`; the set is read at every pairing,
/// so a test changes who may pair by changing the set.
pub(crate) async fn relay_admitting(
    cl: &Cluster,
    seed: u8,
    admitted: Arc<Mutex<HashSet<PeerId>>>,
) -> PeeroxideRelay {
    let admit: RelayAdmit = Arc::new(move |peer: PeerId| {
        admitted
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&peer)
    });
    let opts = RelayOpts {
        assume_open: true,
        ..relay_opts(relay_key_pair(&secret(seed)), admit)
    };
    PeeroxideFactory {
        bootstrap: Some(cl.bootstrap()),
        ..Default::default()
    }
    .create_relay_with(opts)
    .await
    .expect("admitting relay came up")
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

/// Whether `relay` matches at least one pairing, i.e. carries a link, within `limit`.
pub(crate) async fn matched_within(relay: &PeeroxideRelay, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    loop {
        if relay.stats().pairings_matched >= 1 {
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
