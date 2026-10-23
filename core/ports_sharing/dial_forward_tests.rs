//! Tests of `forward_to_peer_port` and `request_port_stream` on `NetStack` over `MemNet`: bytes through the ports service, a peer refusal
//! closes TCP with a `[port]` line, a truncated reply gives the text `reply not received: early eof`.

use std::sync::Arc;
use std::time::Duration;

use crate::node::output::{log_page, log_summary};
use crate::ports_sharing::{
    port_code_text, port_service, Audience, Published, PublishedPortsTable,
    PORT_CODE_NOT_PUBLISHED, PORT_FRAME_VERSION,
};
use crate::transport::net::facade::test_support::{pid, secret, within, Probe, STEP};
use crate::transport::net::{
    BoxFuture, MemNet, NetStack, NetStackBuilder, PeerConn, ServiceHandler, StreamPair, PORT,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use super::*;

/// A TCP pair: the client half and the accepted one.
async fn pair() -> (TcpStream, TcpStream) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let c = TcpStream::connect(l.local_addr().unwrap()).await.unwrap();
    (c, l.accept().await.unwrap().0)
}

async fn assert_eof(mut client: TcpStream) {
    let mut buf = [0u8; 8];
    let r = tokio::time::timeout(Duration::from_secs(1), client.read(&mut buf))
        .await
        .expect("the client did not see the close");
    assert!(matches!(r, Ok(0) | Err(_)));
}

mod member_without_route {
    use super::*;

    #[tokio::test]
    async fn unknown_member_closes_fast() {
        // given: the node is in the member list but there is no route to it; deadline 5 s
        let mut fx = setup(Opts {
            peer: None,
            ..Opts::default()
        })
        .await;

        // when: the client waits for the reply from forward_to_peer_port
        // then: TCP is closed in ≤ 1 s, not by the deadline
        fx.then_forward_ends_within(Duration::from_secs(1)).await;
        fx.then_client_sees_close().await;
    }
}

mod serving_peer {
    use super::*;

    #[tokio::test]
    async fn forward_carries_echo_through_service() {
        // given: B serves ports and publishes an echo port, A knows B
        let echo = echo_listener().await;
        let peer = Some(serving(echo));
        let mut fx = setup(Opts {
            peer,
            port: echo,
            ..Opts::default()
        })
        .await;

        // when: the client writes "hello" into the TCP that forward_to_peer_port forwards
        fx.when_client_writes(b"hello").await;

        // then: the same bytes come back as an echo from the TCP on B's side
        assert_eq!(fx.then_client_reads(5).await, b"hello");
    }
}

mod refusing_peer {
    use super::*;

    #[tokio::test]
    async fn refusal_code_closes_tcp() {
        // given: B answers "not published"
        let peer = Some(scripted(&[PORT_FRAME_VERSION, PORT_CODE_NOT_PUBLISHED]));
        let mut fx = setup(Opts {
            peer,
            local: 41_002,
            ..Opts::default()
        })
        .await;

        // when: forward_to_peer_port got a refusal
        fx.then_forward_ends_within(STEP).await;

        // then: a `[port]` line with the code text, the client's TCP is closed, no data
        fx.then_journal_has(port_code_text(PORT_CODE_NOT_PUBLISHED));
        fx.then_client_sees_close().await;
    }
}

mod peer_cuts_answer {
    use super::*;

    #[tokio::test]
    async fn early_eof_answer_text() {
        // given: the peer returns 1 byte of reply and closes the stream
        let net = MemNet::new();
        let (a, _b) = linked(&net, scripted(&[PORT_FRAME_VERSION]));

        // when: A asks for a port
        let pool = PeerConnPool::new(PORT);
        let asked = request_port_stream(&a, &pool, pid(2), 80);
        let asked = within("ask_port", asked).await;

        // then: an error with the early-end text
        let err = asked.err().expect("the reply is truncated — expected Err");
        assert_eq!(err.to_string(), "reply not received: early eof");
    }
}

// assumption: `forward_to_peer_port_within(&NetStack, &PeerConnPool, &Members, &Opened, TcpStream, Duration)`,
// `request_port_stream(&NetStack, &PeerConnPool, PeerId, u16) -> Result<(BoxSend, BoxRecv, u8)>`,
// `port_service(members, table) -> Arc<dyn ServiceHandler>` and `PeerConnPool::new(PORT)`.

/// Preconditions: the ports handler on B (`None` — A does not know B's address), the port, `local`, the deadline.
struct Opts {
    peer: Option<Arc<dyn ServiceHandler>>,
    port: u16,
    local: u16,
    deadline: Duration,
}

impl Default for Opts {
    fn default() -> Self {
        let peer: Arc<dyn ServiceHandler> = Probe::new();
        Opts {
            peer: Some(peer),
            port: 80,
            local: 1,
            deadline: Duration::from_secs(5),
        }
    }
}

/// `forward_to_peer_port_within` runs in the background; the client is the TCP half the test reads from and writes to.
struct Fixture {
    client: TcpStream,
    forwarded: JoinHandle<()>,
    local: u16,
    seq_before: u64,
    _keep: (Arc<MemNet>, NetStack, NetStack),
}

async fn setup(opts: Opts) -> Fixture {
    let net = MemNet::new();
    let (a, b) = match opts.peer {
        Some(handler) => linked(&net, handler),
        None => (node(&net, 1, Probe::new()), node(&net, 2, Probe::new())),
    };
    let members = Members::default();
    members.add_verified_conn(pid(2));
    let (client, accepted) = pair().await;
    let target = Opened {
        node: pid(2),
        port: opts.port,
        local: opts.local,
    };
    let (seq_before, deadline, stack) = (log_summary(0).last_seq, opts.deadline, a.clone());
    let forwarded = crate::node::output::spawn(async move {
        let pool = PeerConnPool::new(PORT);
        forward_to_peer_port_within(&stack, &pool, &members, &target, accepted, deadline).await
    });
    let local = opts.local;
    Fixture {
        client,
        forwarded,
        local,
        seq_before,
        _keep: (net, a, b),
    }
}

impl Fixture {
    async fn when_client_writes(&mut self, bytes: &[u8]) {
        self.client
            .write_all(bytes)
            .await
            .expect("write to the client TCP");
    }

    async fn then_client_reads(&mut self, n: usize) -> Vec<u8> {
        let mut buf = vec![0u8; n];
        within("data reached the client", self.client.read_exact(&mut buf))
            .await
            .expect("read from the client TCP");
        buf
    }

    async fn then_forward_ends_within(&mut self, limit: Duration) {
        let ended = tokio::time::timeout(limit, &mut self.forwarded).await;
        let ended = ended.unwrap_or_else(|_| panic!("forward did not finish within {limit:?}"));
        ended.expect("the forward task did not panic");
    }

    async fn then_client_sees_close(self) {
        assert_eof(self.client).await;
    }

    /// The line with the `port` tag of this forward (starting with `local`) contains `fragment`.
    fn then_journal_has(&self, fragment: &str) {
        let head = format!("127.0.0.1:{} →", self.local);
        let lines: Vec<String> = log_page(self.seq_before, 4000)
            .lines
            .into_iter()
            .filter(|l| l.tag == "port" && l.text.contains(&head))
            .map(|l| l.text)
            .collect();
        let found = lines.iter().any(|text| text.contains(fragment));
        assert!(found, "the [port] log has no \"{fragment}\": {lines:?}");
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

/// TCP echo on loopback; returns the port.
async fn echo_listener() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
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
