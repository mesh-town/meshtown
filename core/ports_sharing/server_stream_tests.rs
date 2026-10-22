//! Tests of stream acceptance of `PortServer` on `NetStack` over `MemNet`: the request frame, the reply code, the echo.
//! The request and the reply go over `BoxSend`/`BoxRecv`. The connection accept loop is in `server_tests.rs`.

use std::sync::Arc;
use std::time::Duration;

use crate::ports_sharing::{encode_port_request, Published, PORT_FRAME_VERSION};
use crate::transport::net::facade::test_support::{pid, secret, within, Probe, STEP};
use crate::transport::net::{
    MemNet, NetStack, NetStackBuilder, PeerConn, ServiceHandler, StreamPair, PORT,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

use super::*;

mod non_member {
    use super::*;

    #[tokio::test]
    async fn refuses_non_member() {
        // given: the port is published to everyone and has a listener, but A is not a member of B
        let (listener, port) = silent_listener().await;
        let fx = setup(Opts {
            member: false,
            ..Opts::publishing(&[port])
        })
        .await;

        // when: A asks for a port
        let mut ask = fx.when_opens_stream().await;
        ask.says(&encode_port_request(port)).await;

        // then: refusal, and nobody connected to the port
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_REFUSED]);
        assert!(
            nobody_connected(&listener).await,
            "connected before the refusal"
        );
    }
}

mod unpublished_port {
    use super::*;

    #[tokio::test]
    async fn unpublished_port_code() {
        // given: A is a member of B, the publication table is empty
        let fx = setup(Opts::default()).await;

        // when: A asks for a port that is not in the table
        let mut ask = fx.when_opens_stream().await;
        ask.says(&encode_port_request(8080)).await;

        // then: "not published"
        assert_eq!(
            ask.hears(2).await,
            [PORT_FRAME_VERSION, PORT_CODE_NOT_PUBLISHED]
        );
    }
}

mod nodes_audience {
    use super::*;

    #[tokio::test]
    async fn listed_member_gets_ok() {
        // given: the port is published to selected nodes, A (seed 1) is in the list, the port echoes
        let port = echo_listener().await;
        let fx = setup(Opts {
            audience: Audience::Nodes(vec![pid(1)]),
            ..Opts::publishing(&[port])
        })
        .await;

        // when: A asks for a port
        let mut ask = fx.when_opens_stream().await;
        ask.says(&encode_port_request(port)).await;

        // then: "open" and the echo
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_OK]);
        ask.says(b"ping").await;
        assert_eq!(ask.hears(4).await, b"ping");
    }

    #[tokio::test]
    async fn unlisted_member_refused() {
        // given: the port is published to node pid(3), A is a member but not in the list
        let (listener, port) = silent_listener().await;
        let fx = setup(Opts {
            audience: Audience::Nodes(vec![pid(3)]),
            ..Opts::publishing(&[port])
        })
        .await;

        // when: A asks for a port
        let mut ask = fx.when_opens_stream().await;
        ask.says(&encode_port_request(port)).await;

        // then: refusal, nobody connected to the port
        assert_eq!(ask.hears(2).await, [PORT_FRAME_VERSION, PORT_CODE_REFUSED]);
        assert!(
            nobody_connected(&listener).await,
            "connected before the refusal"
        );
    }
}

mod malformed_request {
    use super::*;

    #[tokio::test]
    async fn bad_version_frame_code() {
        // given: A is a member of B
        let fx = setup(Opts::default()).await;

        // when: A sends a frame of a foreign version
        let mut ask = fx.when_opens_stream().await;
        ask.says(&[9, 0, 80]).await;

        // then: "request could not be parsed"
        assert_eq!(
            ask.hears(2).await,
            [PORT_FRAME_VERSION, PORT_CODE_BAD_REQUEST]
        );
    }

    #[tokio::test]
    async fn early_eof_request_is_bad_request() {
        // given: A is a member of B
        let fx = setup(Opts::default()).await;

        let cuts: [&[u8]; 2] = [&[], &[PORT_FRAME_VERSION, 0]];
        for cut in cuts {
            // when: A sends a stub of a request (0 or 2 bytes of 3) and closes the send side
            let mut ask = fx.when_opens_stream().await;
            ask.says(cut).await;
            ask.finishes().await;

            // then: the reply comes at once, not after REQUEST_TIMEOUT (5 s)
            let got = ask.hears_within(2, Duration::from_secs(1)).await;
            assert_eq!(
                got,
                [PORT_FRAME_VERSION, PORT_CODE_BAD_REQUEST],
                "a stub of {} bytes",
                cut.len()
            );
        }
    }
}

mod published_port {
    use super::*;

    #[tokio::test]
    async fn serves_echo_and_marks_local_ok() {
        // given: A is a member of B, the port is published to everyone, a TCP echo is on it
        let port = echo_listener().await;
        let fx = setup(Opts::publishing(&[port])).await;

        // when: A asks for a port and sends "ping"
        let mut ask = fx.when_opens_stream().await;
        ask.says(&encode_port_request(port)).await;
        let code = ask.hears(2).await;
        ask.says(b"ping").await;

        // then: the reply is "open", "ping" came back as an echo, the port is marked working
        assert_eq!(code, [PORT_FRAME_VERSION, PORT_CODE_OK]);
        assert_eq!(ask.hears(4).await, b"ping");
        assert_eq!(fx.local_state(port), LocalState::Ok);
    }

    #[tokio::test]
    async fn nobody_listens_marks_refused() {
        // given: the port is published but nobody listens on it
        let port = free_port();
        let fx = setup(Opts::publishing(&[port])).await;

        // when: A asks for a port
        let mut ask = fx.when_opens_stream().await;
        ask.says(&encode_port_request(port)).await;

        // then: "port does not respond", Refused in the table
        assert_eq!(
            ask.hears(2).await,
            [PORT_FRAME_VERSION, PORT_CODE_CONNECT_FAILED]
        );
        assert_eq!(fx.local_state(port), LocalState::Refused);
    }

    #[tokio::test]
    async fn streams_in_parallel_on_one_conn() {
        // given: two echo ports; a stream is open on the first and its TCP is held
        let (first_port, second_port) = (echo_listener().await, echo_listener().await);
        let fx = setup(Opts::publishing(&[first_port, second_port])).await;
        let mut first = fx.when_opens_stream().await;
        first.says(&encode_port_request(first_port)).await;
        assert_eq!(
            first.hears(2).await,
            [PORT_FRAME_VERSION, PORT_CODE_OK],
            "the first stream is open"
        );

        // when: a second stream goes over the same connection
        let mut second = fx.when_opens_stream().await;
        second.says(&encode_port_request(second_port)).await;

        // then: the second is answered and works, without waiting for the first to end
        assert_eq!(
            second.hears(2).await,
            [PORT_FRAME_VERSION, PORT_CODE_OK],
            "the accept loop is alive"
        );
        second.says(b"ping").await;
        assert_eq!(second.hears(4).await, b"ping");
    }
}

// assumption: `port_service(members, table) -> Arc<dyn ServiceHandler>` stays in `server.rs`.

/// Test preconditions: A is a member of B or not, which ports B publishes to everyone.
struct Opts {
    member: bool,
    published: Vec<u16>,
    audience: Audience,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            member: true,
            published: Vec::new(),
            audience: Audience::All,
        }
    }
}

impl Opts {
    fn publishing(ports: &[u16]) -> Self {
        Opts {
            published: ports.to_vec(),
            ..Opts::default()
        }
    }
}

/// A (seed 1) is connected to B (seed 2), B has the ports handler on `PORT` with table `table`.
struct Fixture {
    conn: Arc<dyn PeerConn>,
    table: PublishedPortsTable,
    _stacks: (Arc<MemNet>, NetStack, NetStack),
}

/// Client end of the A→B stream.
struct Client(StreamPair);

async fn setup(opts: Opts) -> Fixture {
    let members = Members::default();
    if opts.member {
        members.add_verified_conn(pid(1));
    }
    let table = PublishedPortsTable::new(
        opts.published
            .iter()
            .map(|p| published(*p, opts.audience.clone()))
            .collect(),
    );
    let net = MemNet::new();
    let handler = port_service(members, table.clone());
    let (a, b) = (node(&net, 1, Probe::new()), node(&net, 2, handler));
    a.add_hints(b.invite_addrs()[0].clone());
    let conn = within("A connects to B", a.connect(pid(2), PORT)).await;
    Fixture {
        conn: conn.expect("connect A→B succeeded"),
        table,
        _stacks: (net, a, b),
    }
}

impl Fixture {
    async fn when_opens_stream(&self) -> Client {
        let pair = within("A opens a stream to B", self.conn.open()).await;
        Client(pair.expect("stream is open"))
    }

    fn local_state(&self, port: u16) -> LocalState {
        let rows = self.table.published_with_state();
        rows.into_iter()
            .find(|(p, _)| p.port == port)
            .expect("port is in the table")
            .1
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

    async fn finishes(&mut self) {
        self.0
            .send
            .shutdown()
            .await
            .expect("half-close of the send side");
    }

    async fn hears(&mut self, n: usize) -> Vec<u8> {
        self.hears_within(n, STEP).await
    }

    async fn hears_within(&mut self, n: usize, limit: Duration) -> Vec<u8> {
        let mut buf = vec![0u8; n];
        let read = timeout(limit, self.0.recv.read_exact(&mut buf)).await;
        read.unwrap_or_else(|_| panic!("{n} bytes did not arrive within {limit:?}"))
            .expect("read from the stream");
        buf
    }
}

fn published(port: u16, audience: Audience) -> Published {
    Published {
        port,
        name: format!("p{port}"),
        audience,
    }
}

/// Node `seed` on network `net` with handler `handler` on `PORT`.
fn node(net: &MemNet, seed: u8, handler: Arc<dyn ServiceHandler>) -> NetStack {
    NetStackBuilder::new(secret(seed))
        .transport(net.transport(&secret(seed)))
        .serve(PORT, handler)
        .build()
        .expect("stack was built")
}

/// TCP echo on loopback; returns the port.
async fn echo_listener() -> u16 {
    let (listener, port) = silent_listener().await;
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut read, mut write) = tcp.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    port
}

/// A listener for which the test itself decides whether to accept connections.
async fn silent_listener() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

/// A port nobody listens on.
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Nobody connected to the listener: a connection would sit in the kernel queue and `accept` would return it at once.
async fn nobody_connected(listener: &TcpListener) -> bool {
    timeout(Duration::from_millis(100), listener.accept())
        .await
        .is_err()
}
