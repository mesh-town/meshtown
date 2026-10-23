//! Tests of `dial_peer_port` on `NetStack` over `MemNet`: the class (`PortReach`) and the text of every way a
//! port can be unreachable, and the open stream when the owner lets the caller in.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::exit::PeerConnPool;
use crate::members::membership::Members;
use crate::ports_sharing::{
    dial_peer_port, port_code_text, port_service, Audience, PortDialError, PortReach, PortStream,
    Published, PublishedPortsTable, PORT_CODE_CONNECT_FAILED, PORT_CODE_NOT_PUBLISHED,
    PORT_CODE_REFUSED, PORT_FRAME_VERSION,
};
use crate::transport::net::facade::test_support::{pid, secret, within, Probe, STEP};
use crate::transport::net::{
    BoxFuture, MemNet, NetStack, NetStackBuilder, PeerConn, ServiceHandler, StreamPair, PORT,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const UNKNOWN_CODE: u8 = 6;

mod node_outside_the_network {
    use super::*;

    #[tokio::test]
    async fn is_offline_and_no_stream_is_opened() {
        // given: B is linked and serving, but is not a member
        let streams = Counting::new();
        let fx = setup(Opts {
            peer: Some(streams.handler()),
            member: false,
            ..Opts::default()
        })
        .await;

        // when: A dials B's port
        let err = fx.when_dial_fails().await;

        // then: offline, with its text, and B accepted no stream
        assert_eq!(err.reach, PortReach::Offline);
        assert_eq!(err.text, "node is offline");
        assert_eq!(streams.count(), 0, "a stream was opened to a non-member");
    }
}

mod owner_answers_with_a_code {
    use super::*;

    #[tokio::test]
    async fn refused_means_not_opened() {
        then_class_and_text(PORT_CODE_REFUSED, PortReach::NotOpened).await;
    }

    #[tokio::test]
    async fn not_published_means_not_opened() {
        then_class_and_text(PORT_CODE_NOT_PUBLISHED, PortReach::NotOpened).await;
    }

    #[tokio::test]
    async fn connect_failed_means_no_service() {
        then_class_and_text(PORT_CODE_CONNECT_FAILED, PortReach::NoService).await;
    }

    #[tokio::test]
    async fn unknown_code_means_failed() {
        then_class_and_text(UNKNOWN_CODE, PortReach::Failed).await;
    }
}

mod owner_lets_the_caller_in {
    use super::*;

    #[tokio::test]
    async fn the_stream_carries_bytes_to_the_published_port() {
        // given: B publishes an echo port to everyone
        let echo = echo_listener().await;
        let fx = setup(Opts {
            peer: Some(serving(echo)),
            port: echo,
            ..Opts::default()
        })
        .await;

        // when: A dials it and writes "hello" into the stream
        let mut stream = fx.when_dial().await;
        stream
            .send
            .write_all(b"hello")
            .await
            .expect("write into the stream");

        // then: the same bytes come back
        let mut buf = [0u8; 5];
        within("echo reached the stream", stream.recv.read_exact(&mut buf))
            .await
            .expect("read from the stream");
        assert_eq!(&buf, b"hello");
    }
}

mod owner_stays_silent {
    use super::*;

    #[tokio::test]
    async fn times_out_at_the_deadline() {
        // given: B accepts the stream and never replies; the deadline is 1 s
        let fx = setup(Opts {
            peer: Some(Counting::new().handler()),
            deadline: Duration::from_secs(1),
            ..Opts::default()
        })
        .await;

        // when: A dials
        let err = fx.when_dial_fails().await;

        // then: the time-out class and its text
        assert_eq!(err.reach, PortReach::TimedOut);
        assert_eq!(err.text, "deadline expired (1 s)");
    }
}

/// The owner replies with `code`: the dial fails with the class `reach` and the text of that code.
async fn then_class_and_text(code: u8, reach: PortReach) {
    let peer = Some(scripted(&[PORT_FRAME_VERSION, code]));
    let fx = setup(Opts {
        peer,
        ..Opts::default()
    })
    .await;
    let err = fx.when_dial_fails().await;
    assert_eq!(err.reach, reach, "class for code {code}");
    assert_eq!(err.text, port_code_text(code));
}

/// Preconditions: the ports handler on B, whether B is a member, the port, the deadline.
struct Opts {
    peer: Option<Arc<dyn ServiceHandler>>,
    member: bool,
    port: u16,
    deadline: Duration,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            peer: None,
            member: true,
            port: 80,
            deadline: Duration::from_secs(5),
        }
    }
}

struct Fixture {
    stack: NetStack,
    pool: PeerConnPool,
    members: Members,
    port: u16,
    deadline: Duration,
    _keep: (Arc<MemNet>, NetStack),
}

async fn setup(opts: Opts) -> Fixture {
    let net = MemNet::new();
    let handler = opts.peer.unwrap_or_else(|| Probe::new());
    let (a, b) = linked(&net, handler);
    let members = Members::default();
    if opts.member {
        members.add_verified_conn(pid(2));
    }
    Fixture {
        stack: a,
        pool: PeerConnPool::new(PORT),
        members,
        port: opts.port,
        deadline: opts.deadline,
        _keep: (net, b),
    }
}

impl Fixture {
    async fn dial(&self) -> Result<PortStream, PortDialError> {
        let limit = self.deadline + STEP;
        let dialing = dial_peer_port(
            &self.stack,
            &self.pool,
            &self.members,
            pid(2),
            self.port,
            self.deadline,
        );
        let dialed = tokio::time::timeout(limit, dialing).await;
        dialed.unwrap_or_else(|_| panic!("dial_peer_port ignored its deadline {:?}", self.deadline))
    }

    async fn when_dial(&self) -> PortStream {
        let dialed = self.dial().await;
        dialed.unwrap_or_else(|e| panic!("the dial was refused: {}", e.text))
    }

    async fn when_dial_fails(&self) -> PortDialError {
        let dialed = self.dial().await;
        dialed.err().expect("the dial was expected to fail")
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

/// A (seed 1) knows the address of B (seed 2, `handler` on `PORT`).
fn linked(net: &MemNet, handler: Arc<dyn ServiceHandler>) -> (NetStack, NetStack) {
    let (a, b) = (node(net, 1, Probe::new()), node(net, 2, handler));
    a.add_hints(b.invite_addrs()[0].clone());
    (a, b)
}

/// Ports handler of B: A is a member, `port` is published to everyone.
fn serving(port: u16) -> Arc<dyn ServiceHandler> {
    let members = Members::default();
    members.add_verified_conn(pid(1));
    let row = Published {
        port,
        name: "echo".into(),
        audience: Audience::All,
    };
    port_service(members, PublishedPortsTable::new(vec![row]))
}

/// Peer scenario: on each stream reads 3 bytes of the request, writes `reply` and closes the stream.
struct Scripted(Vec<u8>);

fn scripted(reply: &[u8]) -> Arc<dyn ServiceHandler> {
    Arc::new(Scripted(reply.to_vec()))
}

impl ServiceHandler for Scripted {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let reply = self.0.clone();
        Box::pin(async move {
            while let Ok(StreamPair { mut send, mut recv }) = conn.accept().await {
                let _ = recv.read_exact(&mut [0u8; 3]).await;
                let _ = send.write_all(&reply).await;
            }
        })
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

/// TCP echo on loopback; returns the port.
async fn echo_listener() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    crate::node::output::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            crate::node::output::spawn(async move {
                let (mut read, mut write) = tcp.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    port
}
