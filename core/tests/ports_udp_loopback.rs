//! Integration tests for UDP port publishing: two real nodes on loopback, A publishes a UDP port served by an
//! echo, B opens it on `127.0.0.1:<local>` and a datagram goes there and back.
//!
//! What it does: the end-to-end path (document entry → read at B → stream to A → UDP echo and back), two
//! senders at once, a `Nodes` circle that excludes B (no reply, the failure shows in B's listener state and
//! clears after a success), unpublishing, TCP and UDP on one number, a taken local UDP port.
//! What it does not do: the frame, the session limits and the page — those are unit tests next to the code.
//! Links: `meshtown_core::ports_sharing`; the helpers are `core/tests/common/ports_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;
#[path = "common/ports_pair.rs"]
mod ports_pair;
#[path = "common/socks_udp.rs"]
mod udp;

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use meshtown_core::node::RunningNode;
use meshtown_core::ports_sharing::open::OpenState;
use meshtown_core::ports_sharing::{Audience, Proto};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

use ports_pair::{
    echoes_on_one_port, free_port_both, free_udp_port, last_error, listener_state, opened_as,
    pair_ports, published, seen_ports, udp_roundtrip, wait_for,
};

const REPLY_WAIT: Duration = Duration::from_secs(15);
const SILENCE_WAIT: Duration = Duration::from_secs(5);

/// Who sees the published port.
#[derive(Clone, Copy)]
enum Circle {
    Everyone,
    PublisherOnly,
}

/// A running pair: A publishes `port`, B listens for it on `local`.
struct Udp {
    a: RunningNode,
    b: RunningNode,
    port: u16,
    local: u16,
}

impl Udp {
    fn audience(&self, circle: Circle) -> Audience {
        match circle {
            Circle::Everyone => Audience::All,
            Circle::PublisherOnly => Audience::Nodes(vec![self.a.id]),
        }
    }

    /// A publishes the echo port for `circle`.
    fn when_published_for(&self, circle: Circle) {
        let p = published(Proto::Udp, self.port, self.audience(circle));
        self.a.set_ports_published(vec![p]);
    }

    /// A removes every publication and B's document has dropped the entry.
    async fn when_unpublished(&self) {
        self.a.set_ports_published(vec![]);
        wait_for(
            Duration::from_secs(10),
            "the entry is gone at B",
            || async { seen_ports(&self.b, &self.a).await.is_empty() },
        )
        .await;
    }

    async fn then_echoed(&self, payload: &[u8]) {
        let got = udp_roundtrip(self.local, payload, REPLY_WAIT).await;
        assert_eq!(got.as_deref(), Some(payload), "no echo through the nodes");
    }

    async fn then_silent(&self) {
        let got = udp_roundtrip(self.local, b"anyone?", SILENCE_WAIT).await;
        assert_eq!(got, None, "a reply arrived where none is allowed");
    }

    async fn then_last_error_is(&self, expected: Option<&str>) {
        wait_for(Duration::from_secs(10), "B's last dial error", || async {
            last_error(&self.b, Proto::Udp, self.local).as_deref() == expected
        })
        .await;
    }
}

/// An echo on a free UDP port, published by A for `circle`, opened by B on a free local port.
async fn setup(circle: Circle) -> Udp {
    let echo = udp::spawn_udp_echo(IpAddr::V4(Ipv4Addr::LOCALHOST))
        .await
        .expect("udp echo");
    let (a, b) = pair_ports(vec![]).await;
    let local = free_udp_port().await;
    let s = Udp {
        a,
        b,
        port: echo.port(),
        local,
    };
    s.when_published_for(circle);
    s.b.set_ports_opened(vec![opened_as(&s.a, Proto::Udp, s.port, local)]);
    wait_for(Duration::from_secs(10), "A's entry at B", || async {
        seen_ports(&s.b, &s.a).await == vec![(Proto::Udp, s.port)]
    })
    .await;
    s
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn udp_echo_through_two_nodes() {
    // given: A publishes a UDP echo for everyone, B opened it.
    let s = setup(Circle::Everyone).await;

    // when a datagram goes to 127.0.0.1:<local> at B / then the same bytes come back.
    s.then_echoed(b"hello udp").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_senders_get_their_own_replies() {
    // given: the same pair.
    let s = setup(Circle::Everyone).await;

    // when two sockets send different payloads at once.
    let (one, two) = tokio::join!(
        udp_roundtrip(s.local, b"first sender", REPLY_WAIT),
        udp_roundtrip(s.local, b"second sender", REPLY_WAIT),
    );

    // then each gets its own payload back.
    assert_eq!(one.as_deref(), Some(&b"first sender"[..]));
    assert_eq!(two.as_deref(), Some(&b"second sender"[..]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn node_outside_audience_gets_no_reply() {
    // given: A publishes for itself only, B is outside the circle.
    let s = setup(Circle::PublisherOnly).await;

    // when B's listener gets a datagram / then there is no reply and the listener keeps "not_opened".
    s.then_silent().await;
    s.then_last_error_is(Some("not_opened")).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn refusal_clears_after_the_circle_opens() {
    // given: a refusal is recorded at B.
    let s = setup(Circle::PublisherOnly).await;
    s.then_silent().await;
    s.then_last_error_is(Some("not_opened")).await;

    // when A publishes for everyone and a new sender gets its echo / then the listener has no error.
    s.when_published_for(Circle::Everyone);
    s.then_echoed(b"let me in").await;
    s.then_last_error_is(None).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unpublished_udp_port_stops_answering() {
    // given: a working pair.
    let s = setup(Circle::Everyone).await;
    s.then_echoed(b"before").await;

    // when A removes the publication and the announcer has run / then a new sender gets no reply.
    s.when_unpublished().await;
    s.then_silent().await;
}

/// A publishes TCP and UDP on one number `port`; B opens both on one local number.
struct Both {
    a: RunningNode,
    b: RunningNode,
    port: u16,
    local: u16,
}

async fn setup_both() -> Both {
    let port = echoes_on_one_port().await;
    let tcp = published(Proto::Tcp, port, Audience::All);
    let udp = published(Proto::Udp, port, Audience::All);
    let (a, b) = pair_ports(vec![tcp, udp]).await;
    let local = free_port_both().await;
    b.set_ports_opened(vec![
        opened_as(&a, Proto::Tcp, port, local),
        opened_as(&a, Proto::Udp, port, local),
    ]);
    wait_for(Duration::from_secs(10), "both entries at B", || async {
        seen_ports(&b, &a).await == vec![(Proto::Tcp, port), (Proto::Udp, port)]
    })
    .await;
    Both { a, b, port, local }
}

async fn tcp_echo(local: u16, payload: &[u8]) -> Vec<u8> {
    let mut s = TcpStream::connect(("127.0.0.1", local)).await.unwrap();
    s.write_all(payload).await.unwrap();
    let mut buf = vec![0u8; payload.len()];
    timeout(REPLY_WAIT, s.read_exact(&mut buf))
        .await
        .expect("no TCP echo")
        .unwrap();
    buf
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_and_udp_on_one_number_both_echo() {
    // given: TCP 53-like and UDP on one number, both published and both opened on one local number.
    let s = setup_both().await;

    // when a TCP client and a UDP sender use that local number / then each protocol is served by its own echo.
    assert_eq!(tcp_echo(s.local, b"over tcp").await, b"over tcp");
    let udp_reply = udp_roundtrip(s.local, b"over udp", REPLY_WAIT).await;
    assert_eq!(udp_reply.as_deref(), Some(&b"over udp"[..]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unpublishing_udp_keeps_tcp_of_the_same_number() {
    // given: both protocols work.
    let s = setup_both().await;
    assert_eq!(tcp_echo(s.local, b"warm up").await, b"warm up");

    // when A drops the UDP publication only and the announcer has run.
    s.a.set_ports_published(vec![published(Proto::Tcp, s.port, Audience::All)]);
    wait_for(Duration::from_secs(10), "only TCP at B", || async {
        seen_ports(&s.b, &s.a).await == vec![(Proto::Tcp, s.port)]
    })
    .await;

    // then UDP gets no reply and TCP still echoes.
    assert_eq!(udp_roundtrip(s.local, b"gone", SILENCE_WAIT).await, None);
    assert_eq!(tcp_echo(s.local, b"still here").await, b"still here");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn taken_local_udp_port_is_bind_failed() {
    // given: another program holds the local UDP port.
    let local = free_udp_port().await;
    let _holder = UdpSocket::bind(("127.0.0.1", local)).await.unwrap();
    let (a, b) = pair_ports(vec![]).await;

    // when B opens a UDP port of A on it / then the listener is bind_failed with a text.
    b.set_ports_opened(vec![opened_as(&a, Proto::Udp, 9, local)]);
    wait_for(Duration::from_secs(5), "bind_failed", || async {
        matches!(listener_state(&b, Proto::Udp, local), Some(OpenState::BindFailed(t)) if !t.is_empty())
    })
    .await;
}
