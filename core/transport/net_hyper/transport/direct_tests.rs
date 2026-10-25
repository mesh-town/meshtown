//! Dialing by an address hint while the DHT is dead (a node whose bootstrap is unreachable).

use super::CONNECT_LIMIT;
use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::contract::transport_cases::{echo_mismatches, spawn_echo};
use crate::transport::net::contract::STEP;
use crate::transport::net::error::NetError;
use crate::transport::net::transport::Transport;
use crate::transport::net_hyper::test_cluster::cluster;
use crate::transport::net_hyper::test_direct::{
    connect_within, direct_addr, ip_of, isolated, serving,
};
use std::time::Duration;
use tokio::time::timeout;

const FIVE_SECONDS: Duration = Duration::from_secs(5);

mod dead_dht {
    use super::*;

    #[tokio::test]
    async fn reaches_a_peer_at_its_invite_address_within_five_seconds() {
        // given
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        let n = isolated(2).await;
        let invite = direct_addr(&s).await;
        // when
        let res = connect_within(&n, &invite, FIVE_SECONDS).await;
        // then
        let conn = res.unwrap_or_else(|e| panic!("direct dial failed: {e}"));
        assert_eq!(conn.link().remote, Some(ip_of(&invite)));
    }

    #[tokio::test]
    async fn uses_hints_given_before_connect() {
        // given
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        let n = isolated(2).await;
        n.add_hints(&direct_addr(&s).await);
        // when
        let res = connect_within(&n, &PeerAddr::new(s.me()), FIVE_SECONDS).await;
        // then
        assert!(res.is_ok(), "hints from add_hints were not dialed");
    }

    #[tokio::test]
    async fn second_stream_goes_by_the_proven_address() {
        // given: a session opened by the invite address, the DHT cannot find the peer
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        let n = isolated(2).await;
        let conn = connect_within(&n, &direct_addr(&s).await, FIVE_SECONDS)
            .await
            .expect("first connection");
        let (_, theirs) = timeout(STEP, s.accept())
            .await
            .expect("accept deadline")
            .expect("incoming");
        let _echo = spawn_echo(theirs);
        // when
        let bad = echo_mismatches(&conn, 2, 4096, Duration::from_secs(15)).await;
        // then
        assert!(bad.is_empty(), "echo of streams {bad:?} did not match");
    }
}

mod proven_addr {
    use super::*;

    #[tokio::test]
    async fn is_the_address_of_a_direct_win() {
        // given
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        let n = isolated(2).await;
        let invite = direct_addr(&s).await;
        // when
        let _conn = connect_within(&n, &invite, FIVE_SECONDS)
            .await
            .expect("direct connection");
        // then
        assert_eq!(n.proven_addr(s.me()), Some(ip_of(&invite)));
        assert_eq!(
            s.proven_addr(n.me()),
            None,
            "an incoming session proves nothing"
        );
    }

    #[tokio::test]
    async fn is_dropped_after_a_failed_direct_dial() {
        // given: a direct session was made, then the peer went away
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        let n = isolated(2).await;
        let _conn = connect_within(&n, &direct_addr(&s).await, FIVE_SECONDS)
            .await
            .expect("direct connection");
        s.close().await;
        // when
        let res = connect_within(&n, &PeerAddr::new(s.me()), CONNECT_LIMIT + FIVE_SECONDS).await;
        // then
        assert!(res.is_err(), "a closed peer must not be reachable");
        assert_eq!(n.proven_addr(s.me()), None);
    }
}

mod local_addr {
    use super::*;

    #[tokio::test]
    async fn of_a_loopback_node_has_no_hints() {
        // given: a loopback node is ephemeral, so it has no address to give
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        // when
        let addr = s.local_addr();
        // then
        assert_eq!(addr.id, s.me());
        assert!(addr.hints.is_empty(), "hints: {:?}", addr.hints);
    }
}

mod own_id {
    use super::*;

    #[tokio::test]
    async fn is_no_route_even_with_hints() {
        // given
        let n = isolated(1).await;
        let me = PeerAddr {
            id: n.me(),
            hints: vec![AddrHint::Ip("127.0.0.1:9".parse().unwrap())],
        };
        n.add_hints(&me);
        // when
        let res = connect_within(&n, &PeerAddr::new(n.me()), FIVE_SECONDS).await;
        // then
        assert!(matches!(res.err(), Some(NetError::NoRoute)));
        assert_eq!(n.proven_addr(n.me()), None);
    }
}
