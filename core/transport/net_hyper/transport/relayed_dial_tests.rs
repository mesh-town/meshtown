//! Dials with relay candidates on a loopback cluster: the direct path is tried first, a broken one falls back to
//! a relay of a member, a failed relay moves the dial to the next member. A broken direct path is made with the
//! test switch `set_dial_faults_for_tests`.

use super::*;
use crate::transport::net::addr::PeerAddr;
use crate::transport::net::contract::pid;
use crate::transport::net::error::NetResult;
use crate::transport::net::id::PeerId;
use crate::transport::net::link::PathKind;
use crate::transport::net::transport::{PeerConn, Transport};
use crate::transport::net_hyper::relay::RelayCandidate;
use crate::transport::net_hyper::test_cluster::{cluster, Cluster};
use crate::transport::net_hyper::test_direct::{connect_within, serving};
use crate::transport::net_hyper::test_relay::{
    asked_within, candidate, fixed_source, relay_on, relayed_via,
};
use std::sync::Arc;
use std::time::Duration;
use townjump_dht_peeroxide::PeeroxideRelay;

const DIAL: Duration = Duration::from_secs(20);
const QUIET: Duration = Duration::from_secs(2);
const ASKED: Duration = Duration::from_secs(5);

mod direct_path_works {
    use super::*;

    #[tokio::test]
    async fn direct_path_never_asks_the_relay() {
        // given
        let fx = setup(Source::Members(1)).await;
        // when
        let conn = fx.dial().await.expect("direct connection");
        // then
        assert!(matches!(conn.link().path, PathKind::Direct));
        assert!(
            !asked_within(&fx.relays[0], QUIET).await,
            "the relay was asked although the direct path works"
        );
    }
}

mod direct_path_is_broken {
    use super::*;

    #[tokio::test]
    async fn without_source_fails_like_before() {
        // given
        let fx = setup(Source::None).await;
        fx.break_direct(&[]);
        // when
        let res = fx.dial().await;
        // then
        assert!(res.is_err(), "no relay source, yet the dial succeeded");
    }

    #[tokio::test]
    async fn empty_source_fails_like_before() {
        // given
        let fx = setup(Source::Members(0)).await;
        fx.break_direct(&[]);
        // when
        let res = fx.dial().await;
        // then
        assert!(res.is_err(), "no candidates, yet the dial succeeded");
    }

    #[tokio::test]
    async fn relayed_link_is_labelled_with_the_member() {
        // given
        let fx = setup(Source::Members(1)).await;
        fx.break_direct(&[]);
        // when
        let conn = fx.dial().await.expect("connection through the relay");
        // then
        let path = conn.link().path;
        assert!(relayed_via(&path, fx.members[0]), "path is {path:?}");
        assert!(
            asked_within(&fx.relays[0], ASKED).await,
            "the relay of the member was not asked to pair"
        );
    }

    #[tokio::test]
    async fn failed_relay_moves_to_the_next_member() {
        // given: the first member's relay is down, the second works
        let fx = setup(Source::Members(2)).await;
        fx.break_direct(&[0]);
        // when
        let first = fx.dial().await;
        let second = fx.dial().await.expect("second dial");
        // then
        assert!(
            first.is_err(),
            "the first dial went through a failing relay"
        );
        let path = second.link().path;
        assert!(relayed_via(&path, fx.members[1]), "path is {path:?}");
    }

    #[tokio::test]
    async fn relayed_peer_skips_the_direct_phase() {
        // given
        let fx = setup(Source::Members(1)).await;
        fx.break_direct(&[]);
        // when
        let first = fx.dial().await.expect("first dial");
        let second = fx.dial().await.expect("second dial");
        // then
        for conn in [&first, &second] {
            let path = conn.link().path;
            assert!(relayed_via(&path, fx.members[0]), "path is {path:?}");
        }
        assert_eq!(
            fx.a.direct_phase_runs_for_tests(),
            1,
            "the direct phase ran again"
        );
    }
}

// --- setup and helpers ---

/// Which relay candidates the dialing node gets.
enum Source {
    /// No source at all: relay is off.
    None,
    /// A source with this many members, each with a relay of its own, in this order.
    Members(usize),
}

struct Fixture {
    a: Arc<HyperTransport>,
    b: Arc<HyperTransport>,
    /// Ids of the members that run the relays.
    members: Vec<PeerId>,
    /// The relay nodes, in the order of `members`; kept alive with the fixture.
    relays: Vec<PeeroxideRelay>,
    _cluster: Cluster,
}

/// Nodes A and B on a loopback cluster, and `Source::Members(n)` relays that A knows as candidates.
async fn setup(source: Source) -> Fixture {
    let cl = cluster(4).await;
    let a = serving(&cl, 1).await;
    let b = serving(&cl, 2).await;
    let count = match source {
        Source::None => 0,
        Source::Members(n) => n,
    };
    let members: Vec<PeerId> = (0..count).map(|i| pid(10 + i as u8)).collect();
    let mut relays = Vec::new();
    for _ in 0..count {
        relays.push(relay_on(&cl).await);
    }
    let list: Vec<RelayCandidate> = members
        .iter()
        .zip(&relays)
        .map(|(member, relay)| candidate(*member, relay))
        .collect();
    if !matches!(source, Source::None) {
        a.set_relay_source(Some(fixed_source(list)));
    }
    Fixture {
        a,
        b,
        members,
        relays,
        _cluster: cl,
    }
}

impl Fixture {
    /// The direct phase of every dial of A fails at once; the relays at these positions fail too.
    fn break_direct(&self, failing_relays: &[usize]) {
        let keys = failing_relays.iter().map(|i| self.relays[*i].public_key());
        self.a.set_dial_faults_for_tests(DialFaults {
            direct_fails: true,
            failing_relays: keys.collect(),
        });
    }

    /// A connects to B.
    async fn dial(&self) -> NetResult<Arc<dyn PeerConn>> {
        let to = PeerAddr::new(self.b.me());
        connect_within(&self.a, &to, DIAL).await
    }
}
