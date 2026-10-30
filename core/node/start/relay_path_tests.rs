//! Both ends of a link that went through a relay report it as relayed, on a LAN-address cluster: the dialer knows
//! the member whose relay it used, the accepting side only knows that the link is relayed. The dialing node has a
//! broken direct path, so the link can only have gone through the relay.

use crate::transport::net::addr::{PeerAddr, RelayRef};
use crate::transport::net::contract::{pid, secret, STEP};
use crate::transport::net::id::PeerId;
use crate::transport::net::link::PathKind;
use crate::transport::net::transport::{PeerConn, Transport};
use crate::transport::net_hyper::test_cluster::{cluster_on, lan_address, Cluster};
use crate::transport::net_hyper::test_direct::{becomes, connect_within, serving};
use crate::transport::net_hyper::test_relay::{
    candidate, fixed_source, matched_within, relay_admitting, relayed_via, PeeroxideRelay,
};
use crate::transport::net_hyper::transport::DialFaults;
use crate::transport::net_hyper::HyperTransport;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

const DIAL: Duration = Duration::from_secs(20);
const MATCHED: Duration = Duration::from_secs(20);
const SETTLED: Duration = Duration::from_secs(5);
/// Seeds of the nodes: A dials B, C runs the relay.
const A: u8 = 1;
const B: u8 = 2;
const C: u8 = 10;

mod vetoed_link {
    use super::*;

    #[tokio::test]
    async fn both_ends_label_a_vetoed_link_relay() {
        // given: A cannot reach B directly, only through the relay of a member
        let fx = setup().await;
        // when
        let (ours, theirs) = fx.link_up().await;
        // then: A names the member, B knows only that the link is relayed
        let dialer_path = ours.link().path;
        assert!(
            relayed_via(&dialer_path, fx.member),
            "dialer path is {dialer_path:?}"
        );
        assert!(
            becomes(SETTLED, || is_unnamed_relay(&theirs.link().path)).await,
            "accepting side path is {:?}",
            theirs.link().path
        );
        assert!(
            matched_within(&fx.relay, MATCHED).await,
            "the relay matched no pairing, so the link did not go through it"
        );
    }
}

// --- setup and helpers ---

struct Fixture {
    a: Arc<HyperTransport>,
    b: Arc<HyperTransport>,
    /// The member that runs the relay (its id is only a label for the path).
    member: PeerId,
    relay: PeeroxideRelay,
    _cluster: Cluster,
}

/// Nodes A and B on a LAN-address cluster and a relay that admits both; A has the relay as its only candidate
/// and a broken direct path. The cluster sits on the LAN address, because on loopback the dial goes direct and
/// the relay proves nothing.
async fn setup() -> Fixture {
    let cluster = cluster_on(&lan_address(), 4).await;
    let (a, b) = (serving(&cluster, A).await, serving(&cluster, B).await);
    let admitted = [A, B].iter().map(|s| secret(*s).peer_id()).collect();
    let relay = relay_admitting(&cluster, C, Arc::new(std::sync::Mutex::new(admitted))).await;
    let member = pid(40);
    a.set_relay_source(Some(fixed_source(vec![candidate(member, &relay)])));
    a.set_dial_faults_for_tests(DialFaults {
        direct_fails: true,
        failing_relays: vec![],
    });
    Fixture {
        a,
        b,
        member,
        relay,
        _cluster: cluster,
    }
}

impl Fixture {
    /// A dials B; returns the connection of A and the connection B accepted.
    async fn link_up(&self) -> (Arc<dyn PeerConn>, Arc<dyn PeerConn>) {
        let ours = connect_within(&self.a, &PeerAddr::new(self.b.me()), DIAL)
            .await
            .expect("connection through the relay");
        let (_, theirs) = timeout(STEP, self.b.accept())
            .await
            .expect("accept deadline")
            .expect("incoming");
        (ours, theirs)
    }
}

fn is_unnamed_relay(path: &PathKind) -> bool {
    matches!(path, PathKind::Relayed(RelayRef::Unnamed))
}
