//! How a node wires in relay through network members, on loopback transports: a broken direct path falls back to
//! the relay of a member of the same network and never to a relay of a foreign one; the mode of the network
//! decides whether a candidate source is set and whether the own advert stays.

use super::*;
use crate::members::access::tally::{Exclusion, How, Ledger};
use crate::members::access::view::{AccessView, Verdicts};
use crate::node::node_state::own_value;
use crate::node::start::relay_candidates::relay_source;
use crate::node::start::relay_candidates::RELAY_ADVERT_KEY;
use crate::transport::net::addr::PeerAddr;
use crate::transport::net::contract::secret;
use crate::transport::net::error::NetResult;
use crate::transport::net::mem::{MemReplica, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::transport::net::transport::{PeerConn, Transport};
use crate::transport::net::{PeerId, ReplicaEntry};
use crate::transport::net_hyper::test_cluster::{cluster, cluster_on, lan_address, Cluster};
use crate::transport::net_hyper::test_direct::{connect_within, serving};
use crate::transport::net_hyper::test_relay::{
    asked_within, fixed_source, relay_on, relayed_via, PeeroxideRelay,
};
use crate::transport::net_hyper::transport::DialFaults;
use bytes::Bytes;
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time::timeout;

const DIAL: Duration = Duration::from_secs(20);
const ASKED: Duration = Duration::from_secs(5);
const WAIT: Duration = Duration::from_secs(5);
/// Seeds of the nodes A and B, who dial each other.
const A: u8 = 1;
const B: u8 = 2;

mod relay_wiring {
    use super::*;

    #[tokio::test]
    async fn broken_direct_goes_through_a_member_of_the_same_network() {
        // given: a relay of member C (network 1) and a relay of node F (network 2); F is live for A and
        // advertised in the replica of network 1, yet the list of network 1 does not admit it
        let fx = Setup::new().await;
        assert!(
            fx.f < fx.c,
            "F must sort before C, or the filter proves nothing"
        );
        // when
        let conn = fx
            .dial_b()
            .await
            .expect("connection through the relay of C");
        // then
        let path = conn.link().path;
        assert!(relayed_via(&path, fx.c), "path is {path:?}");
        assert!(
            asked_within(&fx.relay_c, ASKED).await,
            "the relay of C was not asked"
        );
        assert_eq!(
            fx.relay_f.stats().pairings_requested,
            0,
            "the relay of a foreign network was asked"
        );
    }

    #[tokio::test]
    async fn network_mode_sets_source() {
        // given
        let (_cl, a) = node(A).await;
        // when
        let task = wire_relay(&a, RelayMode::Network, &Members::default(), replica_of(A));
        // then
        assert!(
            a.relay_source_is_set(),
            "no candidate source in the network mode"
        );
        task.abort();
        a.close().await;
    }

    #[tokio::test]
    async fn off_mode_sets_nothing_and_retracts() {
        // given: a source from before and an old advert of this node
        let (_cl, a) = node(A).await;
        a.set_relay_source(Some(fixed_source(vec![])));
        let replica = replica_of(A);
        replica
            .set_own(RELAY_ADVERT_KEY, Bytes::from_static(&[7; 32]))
            .await
            .expect("old advert");
        // when
        let task = wire_relay(&a, RelayMode::Off, &Members::default(), replica.clone());
        // then
        assert!(
            !a.relay_source_is_set(),
            "a candidate source stayed in the off mode"
        );
        assert!(
            timeout(WAIT, task).await.is_ok(),
            "the retract task did not finish"
        );
        assert_eq!(
            advert_of(&replica).await,
            None,
            "the old advert was not retracted"
        );
        a.close().await;
    }
}

mod admit_of {
    use super::*;

    #[test]
    fn excluded_member_is_refused() {
        // given: X was on the list and was excluded by a vote
        let x = peer(30);
        let access = access_of(&[peer(B), x], &[x]);
        // when
        let admit = admit_of(access);
        // then
        assert!(!admit(x), "an excluded member may pair through the relay");
        assert!(admit(peer(B)), "a member in good standing was refused");
    }

    #[test]
    fn ledger_member_without_a_link_is_admitted() {
        // given: B is on the list but has no verified connection to this node
        let members = members_of(&[peer(B)], &[]);
        // when
        let admit = admit_of(members.access());
        // then
        assert!(admit(peer(B)), "a listed member without a link was refused");
        assert!(!admit(peer(31)), "a node outside the list may pair");
    }

    #[test]
    fn admit_follows_new_verdicts() {
        // given: an admission made while the list admitted B
        let access = access_of(&[peer(B)], &[]);
        let admit = admit_of(access.clone());
        assert!(admit(peer(B)), "B is on the list");
        // when: new verdicts come without B
        access.publish(verdicts_of(&[], &[]));
        // then
        assert!(
            !admit(peer(B)),
            "the admission kept answering by old verdicts"
        );
    }

    #[test]
    fn everyone_passes_before_the_first_verdicts() {
        // given: no verdicts published yet
        let access = AccessView::pending();
        // when
        let admit = admit_of(access);
        // then
        assert!(admit(peer(32)), "the relay refused before any verdict");
    }
}

// --- setup and helpers ---

/// A loopback cluster with one serving node of the given seed.
async fn node(seed: u8) -> (Cluster, Arc<HyperTransport>) {
    let cl = cluster(3).await;
    let t = serving(&cl, seed).await;
    (cl, t)
}

/// Nodes A and B on a cluster; A dials B with a broken direct path. Network 1 has the members A, B and C, and C
/// runs a relay. Node F belongs to another network and runs a relay too: A is connected to F and the replica of
/// network 1 holds its advert, but the list of network 1 does not admit F.
struct Setup {
    a: Arc<HyperTransport>,
    b: Arc<HyperTransport>,
    c: PeerId,
    f: PeerId,
    relay_c: PeeroxideRelay,
    relay_f: PeeroxideRelay,
    _cluster: Cluster,
}

impl Setup {
    async fn new() -> Setup {
        let cl = cluster_on(&lan_address(), 5).await;
        let (a, b) = (serving(&cl, A).await, serving(&cl, B).await);
        let (relay_c, relay_f) = (relay_on(&cl).await, relay_on(&cl).await);
        let (f, c) = ordered_pair(20, 21);
        let network_1 = replica_of(A);
        network_1.put_raw(advert(c, &relay_c));
        network_1.put_raw(advert(f, &relay_f));
        let network_2 = replica_of(A);
        network_2.put_raw(advert(f, &relay_f));
        let members = members_of(&[peer(B), c], &[peer(B), c, f]);
        a.set_relay_source(Some(relay_source(members, network_1, peer(A))));
        a.set_dial_faults_for_tests(DialFaults {
            direct_fails: true,
            failing_relays: vec![],
        });
        Setup {
            a,
            b,
            c,
            f,
            relay_c,
            relay_f,
            _cluster: cl,
        }
    }

    async fn dial_b(&self) -> NetResult<Arc<dyn PeerConn>> {
        connect_within(&self.a, &PeerAddr::new(self.b.me()), DIAL).await
    }
}

/// The ids of seeds `x` and `y`, the smaller one first.
fn ordered_pair(x: u8, y: u8) -> (PeerId, PeerId) {
    let (x, y) = (peer(x), peer(y));
    match x < y {
        true => (x, y),
        false => (y, x),
    }
}

/// `Members` of node A: the list admits `admitted` (and A), verified connections are to `connected`.
fn members_of(admitted: &[PeerId], connected: &[PeerId]) -> Members {
    let members = Members::with_access(access_of(admitted, &[]));
    for id in connected {
        members.add_verified_conn(*id);
    }
    members
}

/// A view that has published verdicts: `admitted` are on the list (A too), `excluded` of them are excluded.
fn access_of(admitted: &[PeerId], excluded: &[PeerId]) -> AccessView {
    let view = AccessView::open();
    view.publish(verdicts_of(admitted, excluded));
    view
}

fn verdicts_of(admitted: &[PeerId], excluded: &[PeerId]) -> Verdicts {
    let mut ledger = Ledger::empty();
    ledger.admitted = admitted.iter().copied().chain([peer(A)]).collect();
    for id in excluded {
        let gone = Exclusion {
            how: How::Votes,
            at_micros: 1,
            voters: vec![],
        };
        ledger.excluded.insert(*id, gone);
    }
    Verdicts {
        me: peer(A),
        ledger: Arc::new(ledger),
        blocks: BTreeSet::new(),
    }
}

fn peer(seed: u8) -> PeerId {
    secret(seed).peer_id()
}

/// The advert of `author`: the key of `relay`.
fn advert(author: PeerId, relay: &PeeroxideRelay) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key: RELAY_ADVERT_KEY.to_vec(),
        value: Bytes::copy_from_slice(&relay.public_key()),
        ts_micros: 1_000,
    }
}

/// An empty in-memory replica of a network, owned by the node with the seed `owner`.
fn replica_of(owner: u8) -> Arc<MemReplica> {
    let ctx = SyncCtx {
        secret: secret(owner),
        network_seed: [9; 32],
        store_dir: None,
    };
    MemSyncNet::new()
        .backend(ctx)
        .replica_mem(REPLICA_NET)
        .expect("replica")
}

/// The own advert in the replica; `None` when there is no live record.
async fn advert_of(replica: &MemReplica) -> Option<Bytes> {
    let own = own_value(replica, RELAY_ADVERT_KEY).await.expect("read");
    own.map(|(value, _)| value)
}
