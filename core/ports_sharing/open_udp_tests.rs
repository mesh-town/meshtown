//! Tests of UDP listeners of `Opener` and of the dial results it keeps: binding, a busy local port, re-opening,
//! closing, the ceiling of senders per listener, the last dial error (UDP and TCP).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::members::membership::Members;
use crate::ports_sharing::dial::PortReach;
use crate::ports_sharing::open::{OpenState, Opener};
use crate::ports_sharing::test_support::{
    free_udp_port, linked, scripted, serving_on, spawn_udp_echo,
};
use crate::ports_sharing::{Opened, Proto, PORT_CODE_BAD_REQUEST, PORT_FRAME_VERSION};
use crate::transport::net::facade::test_support::{pid, within};
use crate::transport::net::{BoxFuture, MemNet, NetStack, PeerConn, ServiceHandler};
use tokio::net::{TcpStream, UdpSocket};

/// The number of senders one listener serves at the same time.
const SENDERS: usize = 128;
const LIMIT: Duration = Duration::from_secs(5);

mod udp_entry {
    use super::*;

    #[tokio::test]
    async fn udp_entry_listens_on_loopback() {
        // given: an opener and a free local UDP port
        let (fx, local) = (setup(Peer::Silent, false).await, free_udp_port());

        // when: the entry is opened
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);

        // then: the port is taken and the state is "listening"
        eventually("the UDP port is taken", || udp_taken(local)).await;
        assert_eq!(fx.state(Proto::Udp, local), OpenState::Listening);
    }

    #[tokio::test]
    async fn taken_udp_local_becomes_bind_failed() {
        // given: another socket holds the local port
        let other = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let local = other.local_addr().unwrap().port();
        let fx = setup(Peer::Silent, false).await;

        // when: the entry is opened
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);

        // then: soon the state is "bind failed" with the reason
        eventually("bind failed", || {
            matches!(fx.state(Proto::Udp, local), OpenState::BindFailed(_))
        })
        .await;
        let OpenState::BindFailed(text) = fx.state(Proto::Udp, local) else {
            unreachable!()
        };
        let expected = bind_failed_text(local);
        assert!(
            text.starts_with(&expected),
            "got {text:?}, expected a text starting with {expected:?}"
        );
    }

    #[tokio::test]
    async fn reopened_udp_local_listens_again() {
        // given: a UDP entry that listens
        let (fx, local) = (setup(Peer::Silent, false).await, free_udp_port());
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);
        eventually("the UDP port is taken", || udp_taken(local)).await;

        // when: the same local port is opened for another remote port
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Udp, 54, local)]);
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // then: it listens, and did not fail on the port the old listener was releasing
        assert_eq!(fx.state(Proto::Udp, local), OpenState::Listening);
        assert!(udp_taken(local), "nobody listens on the local port");
    }

    #[tokio::test]
    async fn tcp_and_udp_on_one_local_both_listen() {
        // given: a local number
        let (fx, local) = (setup(Peer::Silent, false).await, free_tcp_port());

        // when: it is opened for TCP and for UDP
        fx.opener.reconcile_listeners(vec![
            entry(Proto::Tcp, 53, local),
            entry(Proto::Udp, 53, local),
        ]);

        // then: both listen
        eventually("the UDP port is taken", || udp_taken(local)).await;
        assert_eq!(fx.state(Proto::Tcp, local), OpenState::Listening);
        assert_eq!(fx.state(Proto::Udp, local), OpenState::Listening);
        assert!(
            TcpStream::connect(("127.0.0.1", local)).await.is_ok(),
            "TCP does not listen"
        );
    }
}

mod closing {
    use super::*;

    #[tokio::test]
    async fn closing_releases_the_udp_local() {
        // given: a UDP entry that listens
        let (fx, local) = (setup(Peer::Silent, false).await, free_udp_port());
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);
        eventually("the UDP port is taken", || udp_taken(local)).await;

        // when: the list becomes empty
        fx.opener.reconcile_listeners(vec![]);

        // then: soon the port is free
        eventually("the UDP port is free", || !udp_taken(local)).await;
    }

    #[tokio::test]
    async fn close_all_releases_the_udp_local() {
        // given: a UDP entry that listens
        let (fx, local) = (setup(Peer::Silent, false).await, free_udp_port());
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);
        eventually("the UDP port is taken", || udp_taken(local)).await;

        // when: the network is closed
        fx.opener.close_all();

        // then: soon the port is free
        eventually("the UDP port is free", || !udp_taken(local)).await;
    }
}

mod senders {
    use super::*;

    #[tokio::test]
    async fn the_128th_sender_is_dialed() {
        // given: a listener whose peer accepts streams and never replies
        let (fx, streams, local) = counting_listener().await;

        // when: 128 different senders send a datagram each
        let _senders = fx.send_from_new_senders(local, SENDERS).await;

        // then: each of them got its own stream
        eventually("128 streams", || streams.count() == SENDERS).await;
    }

    #[tokio::test]
    async fn sender_over_the_limit_is_dropped() {
        // given: 128 senders hold their dials
        let (fx, streams, local) = counting_listener().await;
        let mut senders = fx.send_from_new_senders(local, SENDERS).await;
        eventually("128 streams", || streams.count() == SENDERS).await;

        // when: the 129th sender sends
        senders.extend(fx.send_from_new_senders(local, 1).await);
        tokio::time::sleep(Duration::from_millis(500)).await;

        // then: no dial was added
        assert_eq!(streams.count(), SENDERS, "the 129th sender was dialed");
    }
}

mod dial_result {
    use super::*;

    #[tokio::test]
    async fn dial_failure_is_kept_until_success() {
        // given: the node is not a member (offline) and the datagram of a sender fails the dial
        let echo = spawn_udp_echo().await;
        let (fx, local) = (
            setup(Peer::Serving(Proto::Udp, echo), false).await,
            free_udp_port(),
        );
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Udp, echo, local)]);
        eventually("the UDP port is taken", || udp_taken(local)).await;
        let first = send_from_new_sender(local, b"one").await;
        eventually("offline", || {
            fx.reach(Proto::Udp, local) == Some(PortReach::Offline)
        })
        .await;

        // when: the node becomes a member and a new sender writes
        fx.members.add_verified_conn(pid(2));
        let second = send_from_new_sender(local, b"two").await;

        // then: the echo comes back and the error is gone
        assert_eq!(hears(&second).await, b"two");
        assert_eq!(fx.reach(Proto::Udp, local), None);
        drop(first);
    }

    #[tokio::test]
    async fn old_node_is_reported_unsupported() {
        // given: a member that answers every request with code 6, as a node without UDP ports does
        let (fx, local) = (
            setup(Peer::Scripted(PORT_CODE_BAD_REQUEST), true).await,
            free_udp_port(),
        );
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);
        eventually("the UDP port is taken", || udp_taken(local)).await;

        // when: a sender writes
        let _sender = send_from_new_sender(local, b"x").await;

        // then: the last error says "unsupported"
        eventually("unsupported", || {
            fx.reach(Proto::Udp, local) == Some(PortReach::Unsupported)
        })
        .await;
    }

    #[tokio::test]
    async fn tcp_dial_failure_is_reported() {
        // given: a TCP entry for a node that is not a member
        let (fx, local) = (setup(Peer::Silent, false).await, free_tcp_port());
        fx.opener
            .reconcile_listeners(vec![entry(Proto::Tcp, 80, local)]);

        // when: somebody connects
        let _conn = TcpStream::connect(("127.0.0.1", local)).await.unwrap();

        // then: the last error says "offline"
        eventually("offline", || {
            fx.reach(Proto::Tcp, local) == Some(PortReach::Offline)
        })
        .await;
    }
}

/// What the node on the other side of the opener does.
enum Peer {
    /// Accepts streams and never replies.
    Silent,
    /// Counts streams and never replies.
    Counting(Counting),
    /// Publishes `port` over `proto` to the opener's node.
    Serving(Proto, u16),
    /// Answers every request with `[1, code]`.
    Scripted(u8),
}

/// A (seed 1) holds the opener; B (seed 2) is `peer`. The node B is a member of A if `member`.
struct Fixture {
    opener: Opener,
    members: Members,
    _keep: (Arc<MemNet>, NetStack),
}

async fn setup(peer: Peer, member: bool) -> Fixture {
    let handler: Arc<dyn ServiceHandler> = match peer {
        Peer::Silent => Counting::new().handler(),
        Peer::Counting(c) => c.handler(),
        Peer::Serving(proto, port) => serving_on(proto, port),
        Peer::Scripted(code) => scripted(&[PORT_FRAME_VERSION, code]),
    };
    let net = MemNet::new();
    let (a, b) = linked(&net, handler);
    let members = Members::default();
    if member {
        members.add_verified_conn(pid(2));
    }
    Fixture {
        opener: Opener::new(a, members.clone()),
        members,
        _keep: (net, b),
    }
}

impl Fixture {
    fn row(&self, proto: Proto, local: u16) -> (OpenState, Option<PortReach>) {
        let rows = self.opener.bind_states();
        let row = rows
            .into_iter()
            .find(|(o, _, _)| o.proto == proto && o.local == local);
        let (_, state, reach) = row.expect("the entry is in the list");
        (state, reach)
    }

    fn state(&self, proto: Proto, local: u16) -> OpenState {
        self.row(proto, local).0
    }

    fn reach(&self, proto: Proto, local: u16) -> Option<PortReach> {
        self.row(proto, local).1
    }

    /// `n` new sockets, each sends one datagram to the listener; the sockets are returned to keep them open.
    async fn send_from_new_senders(&self, local: u16, n: usize) -> Vec<UdpSocket> {
        let mut senders = Vec::new();
        for _ in 0..n {
            senders.push(send_from_new_sender(local, b"x").await);
        }
        senders
    }
}

/// A UDP listener towards a peer that counts streams and never replies.
async fn counting_listener() -> (Fixture, Counting, u16) {
    let (streams, local) = (Counting::new(), free_udp_port());
    let fx = setup(Peer::Counting(streams.clone()), true).await;
    fx.opener
        .reconcile_listeners(vec![entry(Proto::Udp, 53, local)]);
    eventually("the UDP port is taken", || udp_taken(local)).await;
    (fx, streams, local)
}

fn entry(proto: Proto, port: u16, local: u16) -> Opened {
    Opened {
        node: pid(2),
        port,
        proto,
        local,
    }
}

/// A new socket sends `payload` to the listener and is returned.
async fn send_from_new_sender(local: u16, payload: &[u8]) -> UdpSocket {
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sender.send_to(payload, ("127.0.0.1", local)).await.unwrap();
    sender
}

async fn hears(sender: &UdpSocket) -> Vec<u8> {
    let mut buf = [0u8; 2048];
    let (n, _) = within("the reply arrives", sender.recv_from(&mut buf))
        .await
        .unwrap();
    buf[..n].to_vec()
}

/// The local UDP port is held by somebody: a second bind fails.
fn udp_taken(local: u16) -> bool {
    std::net::UdpSocket::bind(("127.0.0.1", local)).is_err()
}

fn free_tcp_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// How the reason of a busy UDP local port begins on this system.
fn bind_failed_text(local: u16) -> String {
    if cfg!(target_os = "macos") {
        return "port is in use by another program".to_string();
    }
    format!("cannot take UDP 127.0.0.1:{local}")
}

/// Waits (polling every 20 ms, up to `LIMIT`) until `cond` holds.
async fn eventually(what: &str, mut cond: impl FnMut() -> bool) {
    let start = tokio::time::Instant::now();
    while !cond() {
        assert!(start.elapsed() < LIMIT, "timed out after {LIMIT:?}: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Peer scenario: counts the streams it accepted and keeps them open without ever replying.
#[derive(Clone)]
struct Counting(Arc<AtomicUsize>);

impl Counting {
    fn new() -> Self {
        Counting(Arc::new(AtomicUsize::new(0)))
    }

    fn handler(&self) -> Arc<dyn ServiceHandler> {
        Arc::new(self.clone())
    }

    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl ServiceHandler for Counting {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let seen = self.0.clone();
        Box::pin(async move {
            let mut held = Vec::new();
            while let Ok(pair) = conn.accept().await {
                held.push(pair);
                seen.fetch_add(1, Ordering::SeqCst);
            }
        })
    }
}
