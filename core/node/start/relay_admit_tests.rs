//! A relay that admits only some peers, on a LAN-address cluster: members pair through it, a stranger and a
//! member whose admission was withdrawn are refused, and the relay key outlives a restart. The dialing nodes have
//! a broken direct path, so a link that comes up went through the relay; the relay counts the pairings it matched.

use crate::transport::net::addr::PeerAddr;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::error::NetResult;
use crate::transport::net::id::PeerId;
use crate::transport::net::transport::{PeerConn, Transport};
use crate::transport::net_hyper::keys::relay_key_pair;
use crate::transport::net_hyper::test_cluster::{cluster_on, lan_address, Cluster};
use crate::transport::net_hyper::test_direct::{connect_within, serving};
use crate::transport::net_hyper::test_relay::{
    candidate, fixed_source, matched_within, relay_admitting, relayed_via, PeeroxideRelay,
};
use crate::transport::net_hyper::transport::DialFaults;
use crate::transport::net_hyper::HyperTransport;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DIAL: Duration = Duration::from_secs(20);
const MATCHED: Duration = Duration::from_secs(20);
/// Seeds of the nodes: A and B are members, D is a stranger, C runs the relay.
const A: u8 = 1;
const B: u8 = 2;
const D: u8 = 3;
const C: u8 = 10;

mod admitted_peers {
    use super::*;

    #[tokio::test]
    async fn member_links_through_an_admitting_relay() {
        // given: A and B are on the list of the relay
        let fx = setup(&[A, B]).await;
        // when
        let conn = fx.dial(&fx.a).await.expect("connection through the relay");
        // then
        let path = conn.link().path;
        assert!(relayed_via(&path, fx.member), "path is {path:?}");
        assert!(
            matched_within(&fx.relay, MATCHED).await,
            "the relay matched no pairing, so the link did not go through it"
        );
    }
}

mod refused_peers {
    use super::*;

    #[tokio::test]
    async fn stranger_is_refused_by_the_relay() {
        // given: D is not on the list of the relay
        let fx = setup(&[A, B]).await;
        let stranger = fx.node(D).await;
        // when
        let res = fx.dial(&stranger).await;
        // then
        assert!(res.is_err(), "a stranger linked through the relay");
        assert_eq!(matched(&fx), 0, "the relay paired a stranger");
    }

    #[tokio::test]
    async fn revoked_member_is_refused_on_the_next_pairing() {
        // given: A linked through the relay while it was admitted
        let fx = setup(&[A, B]).await;
        let first = fx.dial(&fx.a).await.expect("first connection");
        assert!(matched_within(&fx.relay, MATCHED).await, "no first pairing");
        let before = matched(&fx);
        // when: A is taken off the list and dials again
        fx.revoke(A);
        let second = fx.dial(&fx.a).await;
        // then
        assert!(second.is_err(), "a revoked member linked through the relay");
        assert_eq!(matched(&fx), before, "the relay paired a revoked member");
        drop(first);
    }

    #[tokio::test]
    async fn peer_off_the_list_cannot_be_reached_through_the_relay() {
        // given: B, the callee, is not on the list; A is
        let fx = setup(&[A]).await;
        // when
        let res = fx.dial(&fx.a).await;
        // then
        assert!(res.is_err(), "the relay paired with a callee off the list");
        assert_eq!(matched(&fx), 0, "the relay paired a callee off the list");
    }
}

mod relay_key {
    use super::*;

    #[tokio::test]
    async fn relay_key_survives_a_restart() {
        // given: a relay of the node with seed C, which then stops
        let cl = cluster_on(&lan_address(), 4).await;
        let first = relay_admitting(&cl, C, Default::default()).await;
        let key = first.public_key();
        drop(first);
        // when: the node starts its relay again from the same seed
        let again = relay_admitting(&cl, C, Default::default()).await;
        // then
        assert_eq!(again.public_key(), key, "the relay key changed");
        assert_eq!(key, relay_key_pair(&secret(C)).public_key);
    }
}

// --- setup and helpers ---

struct Fixture {
    a: Arc<HyperTransport>,
    b: Arc<HyperTransport>,
    /// The member that runs the relay (its id is only a label for the path).
    member: PeerId,
    relay: PeeroxideRelay,
    /// Who the relay admits; read at every pairing.
    admitted: Arc<Mutex<HashSet<PeerId>>>,
    cluster: Cluster,
}

/// Nodes A and B on a LAN-address cluster, a relay that admits the nodes with these seeds, and a candidate source
/// for A with that relay. A's direct path is broken. The cluster sits on the LAN address, because on loopback
/// the dial goes direct and the relay proves nothing.
async fn setup(admitted: &[u8]) -> Fixture {
    let cluster = cluster_on(&lan_address(), 4).await;
    let (a, b) = (serving(&cluster, A).await, serving(&cluster, B).await);
    let admitted: Arc<Mutex<HashSet<PeerId>>> = Arc::new(Mutex::new(
        admitted.iter().map(|s| secret(*s).peer_id()).collect(),
    ));
    let relay = relay_admitting(&cluster, C, admitted.clone()).await;
    let member = pid(40);
    let fx = Fixture {
        a,
        b,
        member,
        relay,
        admitted,
        cluster,
    };
    fx.equip(&fx.a);
    fx
}

impl Fixture {
    /// Gives `node` the relay as its only candidate and breaks its direct path.
    fn equip(&self, node: &HyperTransport) {
        node.set_relay_source(Some(fixed_source(vec![candidate(
            self.member,
            &self.relay,
        )])));
        node.set_dial_faults_for_tests(DialFaults {
            direct_fails: true,
            failing_relays: vec![],
        });
    }

    /// A further node of the cluster, equipped like A.
    async fn node(&self, seed: u8) -> Arc<HyperTransport> {
        let node = serving(&self.cluster, seed).await;
        self.equip(&node);
        node
    }

    /// Takes the node with this seed off the relay's list.
    fn revoke(&self, seed: u8) {
        self.admitted
            .lock()
            .unwrap()
            .remove(&secret(seed).peer_id());
    }

    /// `from` connects to B.
    async fn dial(&self, from: &HyperTransport) -> NetResult<Arc<dyn PeerConn>> {
        connect_within(from, &PeerAddr::new(self.b.me()), DIAL).await
    }
}

/// How many pairings the relay has matched so far.
fn matched(fx: &Fixture) -> u64 {
    fx.relay.stats().pairings_matched
}
