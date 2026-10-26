//! Tests of UDP requests to `PortServer` on `NetStack` over `MemNet`: the circle, the protocol of the
//! publication, the version 2 frame for TCP, the ceiling of UDP sessions. A UDP echo stands on loopback.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::members::membership::Members;
use crate::ports_sharing::server::PortServer;
use crate::ports_sharing::test_support::{datagram_frame, node, spawn_udp_echo, tcp_echo};
use crate::ports_sharing::{
    encode_port_request, port_service, Audience, LocalState, PortRequest, Proto, Published,
    PublishedPortsTable, PORT_CODE_BUSY, PORT_CODE_NOT_PUBLISHED, PORT_CODE_OK, PORT_CODE_REFUSED,
    PORT_FRAME_VERSION,
};
use crate::transport::net::facade::test_support::{pid, within, Probe, STEP};
use crate::transport::net::{MemNet, NetStack, PeerConn, ServiceHandler, StreamPair, PORT};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

mod udp_request {
    use super::*;

    #[tokio::test]
    async fn member_in_audience_gets_ok_and_echo() {
        // given: a UDP echo published to everyone
        let echo = spawn_udp_echo().await;
        let fx = setup(Opts::publishing(Proto::Udp, echo)).await;

        // when: A asks for it over UDP and sends a framed datagram
        let mut ask = fx.when_opens_stream().await;
        ask.says(&udp_request(echo)).await;
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_OK]);
        ask.says(&datagram_frame(b"ping")).await;

        // then: the same frame comes back
        assert_eq!(ask.hears(8).await, datagram_frame(b"ping"));
    }

    #[tokio::test]
    async fn node_outside_audience_is_refused() {
        // given: the port is published to node 3 only; A (seed 1) is a member but not in the list
        let echo = spawn_udp_echo().await;
        let fx = setup(Opts {
            audience: Audience::Nodes(vec![pid(3)]),
            ..Opts::publishing(Proto::Udp, echo)
        })
        .await;

        // when: A asks for the port
        let mut ask = fx.when_opens_stream().await;
        ask.says(&udp_request(echo)).await;

        // then: refusal, and the stream is closed
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_REFUSED]);
        assert!(
            ask.is_closed().await,
            "the stream stays open after a refusal"
        );
    }

    #[tokio::test]
    async fn tcp_only_port_is_not_published_for_udp() {
        // given: the number is published over TCP only
        let fx = setup(Opts::publishing(Proto::Tcp, 53)).await;

        // when: A asks for it over UDP
        let mut ask = fx.when_opens_stream().await;
        ask.says(&udp_request(53)).await;

        // then: "not published"
        assert_eq!(
            ask.hears(2).await,
            [PORT_FRAME_VERSION, PORT_CODE_NOT_PUBLISHED]
        );
    }

    #[tokio::test]
    async fn the_session_does_not_touch_the_local_state() {
        // given: a UDP echo published to everyone
        let echo = spawn_udp_echo().await;
        let fx = setup(Opts::publishing(Proto::Udp, echo)).await;

        // when: A opens a session
        let mut ask = fx.when_opens_stream().await;
        ask.says(&udp_request(echo)).await;
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_OK]);

        // then: UDP has no local check, the state stays unknown
        assert_eq!(fx.local_state(Proto::Udp, echo), LocalState::Unknown);
    }
}

mod tcp_request {
    use super::*;

    #[tokio::test]
    async fn udp_only_port_is_not_published_for_tcp() {
        // given: the number is published over UDP only
        let fx = setup(Opts::publishing(Proto::Udp, 53)).await;

        // when: A asks for it over TCP (version 1)
        let mut ask = fx.when_opens_stream().await;
        ask.says(&tcp_request(53)).await;

        // then: "not published"
        assert_eq!(
            ask.hears(2).await,
            [PORT_FRAME_VERSION, PORT_CODE_NOT_PUBLISHED]
        );
    }

    #[tokio::test]
    async fn version_2_tcp_request_is_served_as_tcp() {
        // given: a TCP echo published to everyone
        let echo = tcp_echo().await;
        let fx = setup(Opts::publishing(Proto::Tcp, echo)).await;

        // when: A asks with a version 2 frame `[2, 0, hi, lo]`
        let mut ask = fx.when_opens_stream().await;
        ask.says(&version_2_tcp_request(echo)).await;

        // then: "open" and the bytes of the echo
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_OK]);
        ask.says(b"ping").await;
        assert_eq!(ask.hears(4).await, b"ping");
    }

    #[tokio::test]
    async fn tcp_is_not_limited_by_udp_sessions() {
        // given: no UDP session may be served at all
        let echo = tcp_echo().await;
        let fx = setup(Opts {
            sessions: Some(0),
            ..Opts::publishing(Proto::Tcp, echo)
        })
        .await;

        // when: A asks for the TCP port
        let mut ask = fx.when_opens_stream().await;
        ask.says(&tcp_request(echo)).await;

        // then: it is served
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_OK]);
    }
}

mod session_ceiling {
    use super::*;

    #[tokio::test]
    async fn last_free_permit_gets_ok() {
        // given: one permit left
        let echo = spawn_udp_echo().await;
        let fx = setup(Opts {
            sessions: Some(1),
            ..Opts::publishing(Proto::Udp, echo)
        })
        .await;

        // when: A asks
        let (reply, _ask) = fx.when_session(echo).await;

        // then: "open"
        assert_eq!(reply, [PORT_FRAME_VERSION, PORT_CODE_OK]);
    }

    #[tokio::test]
    async fn session_over_the_limit_gets_busy() {
        // given: all permits are taken
        let echo = spawn_udp_echo().await;
        let fx = setup(Opts {
            sessions: Some(1),
            ..Opts::publishing(Proto::Udp, echo)
        })
        .await;
        let _held = fx.takes_all_permits().await;

        // when: a new request arrives
        let mut ask = fx.when_opens_stream().await;
        ask.says(&udp_request(echo)).await;

        // then: "too many sessions", the stream is closed
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_BUSY]);
        assert!(
            ask.is_closed().await,
            "the stream stays open after the busy code"
        );
    }

    #[tokio::test]
    async fn released_permit_lets_the_next_session_in() {
        // given: the only permit is taken and a request got "busy"
        let echo = spawn_udp_echo().await;
        let fx = setup(Opts {
            sessions: Some(1),
            ..Opts::publishing(Proto::Udp, echo)
        })
        .await;
        let held = fx.takes_all_permits().await;
        let (refused, _) = fx.when_session(echo).await;
        assert_eq!(refused, [PORT_FRAME_VERSION, PORT_CODE_BUSY]);

        // when: the permit is released
        drop(held);

        // then: the next request is served
        let (served, _ask) = fx.when_session(echo).await;
        assert_eq!(served, [PORT_FRAME_VERSION, PORT_CODE_OK]);
    }

    #[tokio::test]
    async fn the_ceiling_is_256_sessions() {
        // given: the default server, 256 sessions are open
        let echo = spawn_udp_echo().await;
        let fx = setup(Opts::publishing(Proto::Udp, echo)).await;
        let mut open = Vec::new();
        for _ in 0..256 {
            let mut ask = fx.when_opens_stream().await;
            ask.says(&udp_request(echo)).await;
            assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_OK]);
            open.push(ask);
        }

        // when: the 257th asks
        let mut over = fx.when_opens_stream().await;
        over.says(&udp_request(echo)).await;

        // then: "busy"
        assert_eq!(over.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_BUSY]);
    }
}

/// Test preconditions: the publication, its circle and the number of UDP session permits.
struct Opts {
    published: Published,
    audience: Audience,
    sessions: Option<usize>,
}

impl Opts {
    fn publishing(proto: Proto, port: u16) -> Self {
        let audience = Audience::All;
        let published = Published {
            proto,
            port,
            name: "p".into(),
            audience: audience.clone(),
        };
        Opts {
            published,
            audience,
            sessions: None,
        }
    }
}

/// A (seed 1) is connected to B (seed 2), a member of B, with the ports handler on `PORT`.
struct Fixture {
    conn: Arc<dyn PeerConn>,
    table: PublishedPortsTable,
    permits: Option<Arc<Semaphore>>,
    _stacks: (Arc<MemNet>, NetStack, NetStack),
}

/// Client end of the A→B stream.
struct Client(StreamPair);

async fn setup(opts: Opts) -> Fixture {
    let members = Members::default();
    members.add_verified_conn(pid(1));
    let row = Published {
        audience: opts.audience,
        ..opts.published
    };
    let table = PublishedPortsTable::new(vec![row]);
    let permits = opts.sessions.map(|n| Arc::new(Semaphore::new(n)));
    let handler: Arc<dyn ServiceHandler> = match &permits {
        None => port_service(members, table.clone()),
        Some(p) => Arc::new(PortServer {
            members,
            table: table.clone(),
            udp_sessions: p.clone(),
        }),
    };
    let net = MemNet::new();
    let (a, b) = (node(&net, 1, Probe::new()), node(&net, 2, handler));
    a.add_hints(b.invite_addrs()[0].clone());
    let conn = within("A connects to B", a.connect(pid(2), PORT)).await;
    Fixture {
        conn: conn.expect("connect A→B succeeded"),
        table,
        permits,
        _stacks: (net, a, b),
    }
}

impl Fixture {
    async fn when_opens_stream(&self) -> Client {
        let pair = within("A opens a stream to B", self.conn.open()).await;
        Client(pair.expect("stream is open"))
    }

    /// Opens a stream, asks for the UDP `port`; returns the two reply bytes and the stream (still open).
    async fn when_session(&self, port: u16) -> (Vec<u8>, Client) {
        let mut ask = self.when_opens_stream().await;
        ask.says(&udp_request(port)).await;
        (ask.hears(2).await, ask)
    }

    /// Takes every permit of the server's semaphore (the tests that use it set `sessions: Some(n)`).
    async fn takes_all_permits(&self) -> tokio::sync::OwnedSemaphorePermit {
        let sessions = self
            .permits
            .clone()
            .expect("the test sets the number of permits");
        let n = u32::try_from(sessions.available_permits()).unwrap();
        sessions
            .acquire_many_owned(n)
            .await
            .expect("semaphore is open")
    }

    fn local_state(&self, proto: Proto, port: u16) -> LocalState {
        let rows = self.table.published_with_state();
        let row = rows
            .into_iter()
            .find(|(p, _)| p.proto == proto && p.port == port);
        row.expect("the port is in the table").1
    }
}

impl Client {
    async fn says(&mut self, bytes: &[u8]) {
        self.0
            .send
            .write_all(bytes)
            .await
            .expect("write to the stream");
    }

    async fn hears(&mut self, n: usize) -> Vec<u8> {
        let mut buf = vec![0u8; n];
        let read = timeout(STEP, self.0.recv.read_exact(&mut buf)).await;
        read.unwrap_or_else(|_| panic!("{n} bytes did not arrive within {STEP:?}"))
            .expect("read from the stream");
        buf
    }

    /// The server ended the stream: the next read is the end of data (or an error).
    async fn is_closed(&mut self) -> bool {
        let mut b = [0u8; 1];
        let read = timeout(Duration::from_secs(1), self.0.recv.read(&mut b)).await;
        matches!(read, Ok(Ok(0)) | Ok(Err(_)))
    }
}

fn udp_request(port: u16) -> Vec<u8> {
    encode_port_request(PortRequest {
        proto: Proto::Udp,
        port,
    })
}

fn tcp_request(port: u16) -> Vec<u8> {
    encode_port_request(PortRequest {
        proto: Proto::Tcp,
        port,
    })
}

/// A request of version 2 that names TCP: `[2, 0, hi, lo]`.
fn version_2_tcp_request(port: u16) -> Vec<u8> {
    let p = port.to_be_bytes();
    vec![2, Proto::Tcp.wire(), p[0], p[1]]
}
