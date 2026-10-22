//! Tests of the `mem` connector (`connector.rs`): address, `dial`/`listen`, a foreign key, closing.

use crate::transport::net::mem::hub::{decode_hint, encode_hint};
use crate::transport::net::mem::{MemConnector, MemNet, MEM_HINT_KIND};
use crate::transport::net::{
    AddrHint, Connector, NetError, NetInfo, NodeSecret, PacketPath, PathKind, PeerAddr, PeerId,
    RelayRef,
};
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

mod identity {
    use super::*;

    #[test]
    fn kind_is_mem() {
        let c = MemNet::new().connector(&secret(1));

        assert_eq!(c.kind(), "mem");
    }

    #[test]
    fn local_addr_is_the_node_id_with_one_hint_of_its_own_slot() {
        let c = MemNet::new().connector(&secret(1));

        let addr = c.local_addr();

        assert_eq!(addr.id, peer(1));
        assert_eq!(
            addr.hints.len(),
            1,
            "the connector has one hint: {:?}",
            addr.hints
        );
        let AddrHint::Opaque { kind, .. } = &addr.hints[0] else {
            panic!("the mem hint must be Opaque: {:?}", addr.hints[0]);
        };
        assert_eq!(*kind, MEM_HINT_KIND);
        assert!(
            decode_hint(&addr.hints[0]).is_some(),
            "its own hint cannot be parsed"
        );
    }

    #[test]
    fn net_info_is_the_default() {
        let c = MemNet::new().connector(&secret(1));

        let got = format!("{:?}", c.net_info());

        assert_eq!(got, format!("{:?}", NetInfo::default()));
    }

    #[test]
    fn add_hints_and_set_relays_change_nothing() {
        let c = MemNet::new().connector(&secret(1));
        let before = c.local_addr();

        c.add_hints(&PeerAddr {
            id: peer(2),
            hints: vec![ip_hint()],
        });
        c.set_relays(&[RelayRef::Url("https://relay.test".into())]);

        assert_eq!(c.local_addr(), before);
        assert_eq!(
            format!("{:?}", c.net_info()),
            format!("{:?}", NetInfo::default())
        );
    }
}

mod can_reach {
    use super::*;

    #[test]
    fn an_address_with_only_an_ip_hint_is_unreachable() {
        let c = MemNet::new().connector(&secret(1));
        let to = PeerAddr {
            id: peer(2),
            hints: vec![ip_hint()],
        };

        assert!(!c.can_reach(&to));
    }

    #[test]
    fn an_address_without_hints_is_unreachable() {
        let c = MemNet::new().connector(&secret(1));

        assert!(!c.can_reach(&PeerAddr::new(peer(2))));
    }

    #[test]
    fn an_address_with_a_hint_of_this_net_is_reachable() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));

        assert!(a.can_reach(&b.local_addr()));
    }

    #[test]
    fn a_hint_of_this_net_is_enough_even_if_the_slot_is_dead() {
        let net = MemNet::new();
        let a = net.connector(&secret(1));
        let (net_id, _) = net_and_slot(&a);
        let to = PeerAddr {
            id: peer(2),
            hints: vec![encode_hint(net_id, 9_999)],
        };

        assert!(
            a.can_reach(&to),
            "can_reach does not check the liveness of the slot"
        );
    }

    #[test]
    fn a_hint_of_another_net_is_unreachable() {
        let a = MemNet::new().connector(&secret(1));
        let b = MemNet::new().connector(&secret(2));

        assert!(!a.can_reach(&b.local_addr()));
    }
}

mod dial_to_a_wrong_address {
    use super::*;

    #[tokio::test]
    async fn only_an_ip_hint_is_no_route() {
        let a = MemNet::new().connector(&secret(1));
        let to = PeerAddr {
            id: peer(2),
            hints: vec![ip_hint()],
        };

        let err = dial_err(&a, &to).await;

        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }

    #[tokio::test]
    async fn no_hints_is_no_route() {
        let a = MemNet::new().connector(&secret(1));

        let err = dial_err(&a, &PeerAddr::new(peer(2))).await;

        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }

    #[tokio::test]
    async fn a_slot_that_never_existed_is_no_route() {
        let net = MemNet::new();
        let a = net.connector(&secret(1));
        let (net_id, _) = net_and_slot(&a);
        let to = PeerAddr {
            id: peer(2),
            hints: vec![encode_hint(net_id, 9_999)],
        };

        let err = dial_err(&a, &to).await;

        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }

    #[tokio::test]
    async fn another_key_behind_the_hint_is_wrong_peer() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        let to_c_by_the_hint_of_b = PeerAddr {
            id: peer(3),
            hints: b.local_addr().hints,
        };

        let err = dial_err(&a, &to_c_by_the_hint_of_b).await;

        let NetError::WrongPeer { want, got } = err else {
            panic!("expected WrongPeer, got {err:?}");
        };
        assert_eq!(want, peer(3), "want — the one that was asked for");
        assert_eq!(got, peer(2), "got — the one behind the hint");
    }

    #[tokio::test]
    async fn the_wanted_node_living_elsewhere_does_not_help() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        let _c = net.connector(&secret(3));
        let to_c_by_the_hint_of_b = PeerAddr {
            id: peer(3),
            hints: b.local_addr().hints,
        };

        let err = dial_err(&a, &to_c_by_the_hint_of_b).await;

        assert!(
            matches!(err, NetError::WrongPeer { .. }),
            "expected WrongPeer, got {err:?}"
        );
    }

    #[tokio::test]
    async fn a_matching_hint_among_foreign_ones_connects() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
        let c = net.connector(&secret(3));
        let mut hints = b.local_addr().hints;
        hints.extend(c.local_addr().hints);
        let to_c = PeerAddr { id: peer(3), hints };

        let path = dial_ok(&a, &to_c).await;

        assert_eq!(path.info().peer, peer(3));
        assert_eq!(listen_path(&c).await.info().peer, peer(1));
    }
}

mod transport_net_info {
    use super::*;
    use crate::transport::net::Transport;

    #[tokio::test]
    async fn mem_net_info_is_unknown_and_ready() {
        // given: the mem transport
        let t = MemNet::new().transport(&secret(1));
        // when: we wait for the first network report and ask net_info()
        let ready = timeout(Duration::from_millis(100), t.wait_net_info()).await;
        let info = t.net_info();
        // then: the wait ended at once, and the transport knows nothing about the network
        assert!(
            ready.is_ok(),
            "wait_net_info of mem did not finish within 100 ms"
        );
        assert_eq!(info.net_seen, None, "net_seen of mem: {info:?}");
    }
}

#[path = "connector_dial_tests.rs"]
mod dialed_and_closed;

// ---- helpers ----

fn secret(n: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[n; 32])
}

fn peer(n: u8) -> PeerId {
    secret(n).peer_id()
}

fn ip_hint() -> AddrHint {
    AddrHint::Ip("127.0.0.1:9".parse().unwrap())
}

/// The network and slot of the first hint of the connector.
fn net_and_slot(c: &Arc<MemConnector>) -> (u64, u64) {
    decode_hint(&c.local_addr().hints[0]).expect("the hint of the connector parses")
}

async fn dial_ok(from: &MemConnector, to: &PeerAddr) -> Box<dyn PacketPath> {
    match timeout(Duration::from_secs(2), from.dial(to)).await {
        Ok(Ok(path)) => path,
        Ok(Err(e)) => panic!("dial must succeed, got {e:?}"),
        Err(_) => panic!("dial hung"),
    }
}

async fn dial_err(from: &MemConnector, to: &PeerAddr) -> NetError {
    match timeout(Duration::from_secs(2), from.dial(to)).await {
        Ok(Err(e)) => e,
        Ok(Ok(_)) => panic!("dial was supposed to refuse"),
        Err(_) => panic!("dial hung"),
    }
}

async fn listen_path(c: &MemConnector) -> Box<dyn PacketPath> {
    let got = timeout(Duration::from_secs(2), c.listen()).await;
    got.expect("listen hung").expect("listen returned None")
}

async fn recv_ok(p: &dyn PacketPath) -> Bytes {
    let got = timeout(Duration::from_secs(2), p.recv()).await;
    got.expect("recv hung").expect("recv returned an error")
}
