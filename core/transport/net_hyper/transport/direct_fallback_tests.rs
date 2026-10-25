//! Dialing when the address hint is useless: a dead port, a foreign socket, no hint at all.
//! The dial falls back to the key lookup, and no dial slot stays taken.

use super::DIAL_SLOTS;
use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::transport::Transport;
use crate::transport::net_hyper::test_cluster::cluster;
use crate::transport::net_hyper::test_direct::{
    becomes, connect_within, dead_addr, serving, silent_socket,
};
use std::net::SocketAddr;
use std::time::Duration;

const FIVE_SECONDS: Duration = Duration::from_secs(5);

mod dead_address {
    use super::*;

    #[tokio::test]
    async fn falls_back_to_the_key() {
        // given
        let cl = cluster(3).await;
        let (s, n) = (serving(&cl, 1).await, serving(&cl, 2).await);
        // when
        let res = connect_within(&n, &dead_addr(s.me()), Duration::from_secs(15)).await;
        // then
        let conn = res.unwrap_or_else(|e| panic!("dial by key failed: {e}"));
        assert_eq!(
            conn.link().remote,
            None,
            "a dial by key has no dialed address"
        );
    }
}

mod foreign_socket {
    use super::*;

    #[tokio::test]
    async fn does_not_hold_the_key_dial() {
        // given: the hint leads to a socket that never answers
        let cl = cluster(3).await;
        let (s, n) = (serving(&cl, 1).await, serving(&cl, 2).await);
        let (_socket, at) = silent_socket();
        let hinted = PeerAddr {
            id: s.me(),
            hints: vec![AddrHint::Ip(SocketAddr::V4(at))],
        };
        // when
        let res = connect_within(&n, &hinted, FIVE_SECONDS).await;
        // then
        let conn = res.unwrap_or_else(|e| panic!("dial by key failed: {e}"));
        assert_eq!(conn.link().remote, None);
    }
}

mod no_hints {
    use super::*;

    #[tokio::test]
    async fn connects_by_key_as_before() {
        // given
        let cl = cluster(3).await;
        let (s, n) = (serving(&cl, 1).await, serving(&cl, 2).await);
        // when
        let res = connect_within(&n, &PeerAddr::new(s.me()), Duration::from_secs(15)).await;
        // then
        let conn = res.unwrap_or_else(|e| panic!("dial by key failed: {e}"));
        assert_eq!(conn.link().remote, None);
        assert_eq!(
            n.proven_addr(s.me()),
            None,
            "a dial by key proves no address"
        );
    }
}

mod lost_direct_attempt {
    use super::*;

    #[tokio::test]
    async fn leaves_every_slot_free() {
        // given
        let cl = cluster(3).await;
        let (s, n) = (serving(&cl, 1).await, serving(&cl, 2).await);
        let _conn = connect_within(&n, &dead_addr(s.me()), Duration::from_secs(15))
            .await
            .expect("dial by key");
        // when
        let free = becomes(Duration::from_secs(1), || {
            n.dialer.slots.available_permits() == DIAL_SLOTS
        })
        .await;
        // then
        assert!(
            free,
            "slots still taken: {} of {DIAL_SLOTS} free",
            n.dialer.slots.available_permits()
        );
    }
}
