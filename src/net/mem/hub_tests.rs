//! Tests of the `mem` hub (`hub.rs`): hint, separate networks, node restart, `cut`/`heal`, delay.

use super::*;
use crate::net::mem::{MemConnector, MEM_HINT_KIND};
use crate::net::{
    AddrHint, Connector, NetError, NodeSecret, PacketPath, PeerAddr, PeerId, RelayRef,
};
use bytes::Bytes;
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};

mod hint {
    use super::*;

    mod encoded {
        use super::*;

        #[test]
        fn is_an_opaque_hint_of_the_mem_kind() {
            let AddrHint::Opaque { kind, .. } = encode_hint(1, 2) else {
                panic!("the mem hint must be Opaque");
            };
            assert_eq!(kind, MEM_HINT_KIND);
            assert_eq!(MEM_HINT_KIND, 0x6d65_6d31, "\"mem1\"");
        }

        #[test]
        fn holds_net_then_slot_big_endian_in_16_bytes() {
            let AddrHint::Opaque { bytes, .. } =
                encode_hint(0x0102_0304_0506_0708, 0x1112_1314_1516_1718)
            else {
                panic!("the mem hint must be Opaque");
            };
            let want: Vec<u8> = (0x01..=0x08).chain(0x11..=0x18).collect();
            assert_eq!(bytes, want);
        }

        #[test]
        fn decodes_back_to_the_same_net_and_slot() {
            for (net, slot) in [(1, 1), (7, u64::MAX), (u64::MAX, 0), (0x0102, 0x0304)] {
                let back = decode_hint(&encode_hint(net, slot));
                assert_eq!(back, Some((net, slot)));
            }
        }
    }

    mod foreign {
        use super::*;

        #[test]
        fn another_kind_is_skipped() {
            let hint = AddrHint::Opaque {
                kind: MEM_HINT_KIND + 1,
                bytes: vec![0; 16],
            };
            assert_eq!(decode_hint(&hint), None);
        }

        #[test]
        fn fifteen_bytes_are_skipped() {
            let hint = AddrHint::Opaque {
                kind: MEM_HINT_KIND,
                bytes: vec![0; 15],
            };
            assert_eq!(decode_hint(&hint), None);
        }

        #[test]
        fn seventeen_bytes_are_skipped() {
            let hint = AddrHint::Opaque {
                kind: MEM_HINT_KIND,
                bytes: vec![0; 17],
            };
            assert_eq!(decode_hint(&hint), None);
        }

        #[test]
        fn an_ip_hint_is_skipped() {
            let hint = AddrHint::Ip("127.0.0.1:9".parse().unwrap());
            assert_eq!(decode_hint(&hint), None);
        }

        #[test]
        fn a_relay_hint_is_skipped() {
            let hint = AddrHint::Relay(RelayRef::Url("https://relay.test".into()));
            assert_eq!(decode_hint(&hint), None);
        }
    }
}

mod separate_nets {
    use super::*;

    #[tokio::test]
    async fn dialing_a_node_of_another_net_is_no_route() {
        let (net1, net2) = (MemNet::new(), MemNet::new());
        let a = net1.connector(&secret(1));
        let b = net2.connector(&secret(2));

        let err = dial_err(&a, &b.local_addr()).await;

        assert!(
            matches!(err, NetError::NoRoute),
            "expected NoRoute, got {err:?}"
        );
    }

    #[tokio::test]
    async fn the_same_node_secret_in_two_nets_gets_different_net_ids() {
        let (net1, net2) = (MemNet::new(), MemNet::new());
        let a1 = net1.connector(&secret(1));
        let a2 = net2.connector(&secret(1));

        assert_ne!(
            net_and_slot(&a1).0,
            net_and_slot(&a2).0,
            "the network ids of two MemNet are equal"
        );
    }

    #[tokio::test]
    async fn nodes_of_one_net_share_its_id() {
        let net = MemNet::new();
        let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));

        assert_eq!(net_and_slot(&a).0, net_and_slot(&b).0);
    }
}

#[path = "hub_restart_tests.rs"]
mod restart_of_a_node;

#[path = "hub_cut_tests.rs"]
mod cut_and_heal;

#[path = "hub_delay_tests.rs"]
mod delay;

mod addr_of {
    use super::*;
    use crate::net::Connector;

    #[test]
    fn addr_of_live_slot() {
        // given: a node with a live slot
        let net = MemNet::new();
        let conn = net.connector(&secret(1));
        // when: we ask for the address by key
        let addr = net.addr_of(peer(1)).expect("the slot is alive");
        // then: the key is the same, the hint is the same as the node's own
        assert_eq!(addr.id, peer(1));
        assert_eq!(addr.hints, conn.local_addr().hints);
    }

    #[tokio::test]
    async fn addr_of_none_after_close() {
        // given: a node whose slot is closed; and a node that never existed
        let net = MemNet::new();
        let conn = net.connector(&secret(1));
        conn.close().await;
        // then: there is no address
        assert!(net.addr_of(peer(1)).is_none());
        assert!(net.addr_of(peer(9)).is_none());
    }

    #[test]
    fn process_net_is_shared() {
        assert!(Arc::ptr_eq(&process_net(), &process_net()));
    }
}

// ---- helpers ----

fn secret(n: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[n; 32])
}

fn peer(n: u8) -> PeerId {
    secret(n).peer_id()
}

fn pkt(i: u32) -> Bytes {
    Bytes::copy_from_slice(&i.to_be_bytes())
}

fn num(b: &Bytes) -> u32 {
    u32::from_be_bytes(b[..].try_into().expect("a packet of 4 bytes"))
}

/// The network and slot of the first hint of the connector.
fn net_and_slot(c: &MemConnector) -> (u64, u64) {
    let addr = c.local_addr();
    decode_hint(&addr.hints[0]).expect("the hint of the connector parses")
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

async fn recv_err(p: &dyn PacketPath) -> ErrorKind {
    let got = timeout(Duration::from_secs(2), p.recv()).await;
    got.expect("recv hung")
        .expect_err("recv must return an error")
        .kind()
}

/// A node was restarted: the old connector of A, the new connector of A, the live line of the old A to B.
struct Restart {
    old: Arc<MemConnector>,
    old_addr: PeerAddr,
    new: Arc<MemConnector>,
    other: Arc<MemConnector>,
    old_to_other: Box<dyn PacketPath>,
    other_to_old: Box<dyn PacketPath>,
}

async fn setup_restart() -> Restart {
    let net = MemNet::new();
    let old = net.connector(&secret(1));
    let other = net.connector(&secret(2));
    let old_addr = old.local_addr();
    let old_to_other = dial_ok(&old, &other.local_addr()).await;
    let other_to_old = listen_path(&other).await;
    let new = net.connector(&secret(1));
    Restart {
        old,
        old_addr,
        new,
        other,
        old_to_other,
        other_to_old,
    }
}

/// Two nodes and a line between them: `a_to_b` is the end of A, `b_to_a` is the end of B.
struct Pair {
    net: Arc<MemNet>,
    a_to_b: Box<dyn PacketPath>,
    b_to_a: Box<dyn PacketPath>,
}

async fn setup_pair() -> Pair {
    open_pair(&MemNet::new()).await
}

async fn open_pair(net: &Arc<MemNet>) -> Pair {
    let (a, b) = (net.connector(&secret(1)), net.connector(&secret(2)));
    let a_to_b = dial_ok(&a, &b.local_addr()).await;
    let b_to_a = listen_path(&b).await;
    Pair {
        net: net.clone(),
        a_to_b,
        b_to_a,
    }
}
