//! Tests for the exit handler on the mem stack (`server.rs`): permission, request frame, byte shuttling,
//! end of connection. The domain address and refusals for closed networks are in `server_tests.rs`.

use super::*;
use crate::exit::clients::EXIT_CLIENT_LINGER;
use crate::exit::frame::encode_exit_request;
use crate::http_min::HttpLimits;
use crate::net::facade::test_support::{pid, secret, within, Spy};
use crate::net::mem::MemNet;
use crate::net::service::EXIT;
use crate::net::{NetStack, NetStackBuilder, StreamPair};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

mod refuses {
    use super::*;

    #[tokio::test]
    async fn non_member_gets_refused() {
        // given: the client is not in `Members`
        let fx = setup(Opts {
            member: false,
            ..Opts::default()
        })
        .await;

        // when: it asks for a stream
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the reply is `EXIT_CODE_REFUSED`
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_REFUSED]);
    }

    #[tokio::test]
    async fn disabled_exit_gets_refused() {
        // given: the client is in `Members`, but the exit is off
        let fx = setup(Opts {
            enabled: false,
            ..Opts::default()
        })
        .await;

        // when: it asks for a stream
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;

        // then: the same reply
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_REFUSED]);
    }
}

mod bad_frame {
    use super::*;

    #[tokio::test]
    async fn cut_frame_replies_bad_request() {
        // given: the client sent only half a frame and closed its side
        let fx = setup(Opts::default()).await;
        let frame = request("site.test", 80);

        // when: the server reads the frame to the end of the stream
        let (_pair, reply) = fx.ask_then_close(&frame[..3]).await;

        // then: the reply is `EXIT_CODE_BAD_REQUEST`
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_BAD_REQUEST]);
    }
}

mod ok {
    use super::*;

    #[tokio::test]
    async fn splices_bytes_to_local_site() {
        // given: the domain leads to a local echo
        let fx = setup(Opts {
            site: Site::Echo,
            ..Opts::default()
        })
        .await;

        // when: the client asks for a stream and writes four bytes
        let (mut pair, reply) = fx.ask(&request("site.test", 80)).await;
        pair.send.write_all(b"ping").await.expect("client write");
        let mut back = [0u8; 4];
        within("echo", pair.recv.read_exact(&mut back))
            .await
            .expect("echo read");

        // then: the code is `EXIT_CODE_OK` and the bytes came back
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);
        assert_eq!(&back, b"ping");
    }
}

mod traffic {
    use super::*;

    #[tokio::test]
    async fn exit_handle_counts_through_us() {
        // given: the domain leads to an echo, the exit has a shared speed registry
        let fx = setup(Opts {
            site: Site::Echo,
            ..Opts::default()
        })
        .await;
        let (mut pair, _reply) = fx.ask(&request("site.test", 80)).await;
        pair.send.write_all(&[7u8; 10_000]).await.expect("client write");
        let mut back = [0u8; 10_000];
        within("echo", pair.recv.read_exact(&mut back)).await.expect("echo read");

        // when: we wait at most three seconds
        let seen = tokio::time::timeout(Duration::from_secs(3), async {
            while fx.cfg.traffic.rates_at(Instant::now()).get(&pid(1)).is_none_or(|r| r.through_us_bps == 0) {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        // then: the client has "through us", and "through peer" is zero
        assert!(seen.is_ok(), "speed did not appear within 3 s");
        let rate = fx.cfg.traffic.rates_at(Instant::now())[&pid(1)];
        assert_eq!(rate.through_peer_bps, 0);
    }
}

mod conn_close {
    use super::*;

    #[tokio::test]
    async fn ends_handler_and_forgets_clients() {
        // given: an open stream to a silent site, the exit client is registered
        let fx = setup(Opts {
            site: Site::Silent,
            ..Opts::default()
        })
        .await;
        let (_pair, reply) = fx.ask(&request("site.test", 80)).await;
        assert_eq!(reply, [EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]);
        within("client registered", fx.wait_client_listed()).await;

        // when: the client closes the connection
        fx.conn.close(0, b"done");

        // then: `handle` finishes, the client counter returned to zero, also after the linger
        within("handle finished", fx.handle_ended()).await;
        let later = Instant::now() + EXIT_CLIENT_LINGER + Duration::from_secs(1);
        assert!(
            fx.cfg.clients.active_peers_at(later).is_empty(),
            "no clients left"
        );
    }
}

/// Preconditions: who asks and what site it is.
struct Opts {
    enabled: bool,
    member: bool,
    site: Site,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            enabled: true,
            member: true,
            site: Site::Silent,
        }
    }
}

#[derive(Clone, Copy)]
enum Site {
    /// Returns everything it received.
    Echo,
    /// Accepts a connection and stays silent.
    Silent,
}

/// A client (node 1) with an open connection to an exit (node 2).
struct Fx {
    cfg: ExitConfig,
    conn: Arc<dyn PeerConn>,
    ended: Arc<Semaphore>,
    _stacks: (NetStack, NetStack),
}

impl Fx {
    /// A request: the whole frame, the reply is the first two bytes (up to `STEP`).
    async fn ask(&self, frame: &[u8]) -> (StreamPair, [u8; 2]) {
        let mut pair = within("open", self.conn.open())
            .await
            .expect("stream opened");
        pair.send.write_all(frame).await.expect("frame sent");
        let reply = read_reply(&mut pair.recv).await;
        (pair, reply)
    }

    /// The same, but after writing the client closes its side (the frame is cut off by the end of the stream).
    async fn ask_then_close(&self, frame: &[u8]) -> (StreamPair, [u8; 2]) {
        let mut pair = within("open", self.conn.open())
            .await
            .expect("stream opened");
        pair.send.write_all(frame).await.expect("frame sent");
        pair.send
            .shutdown()
            .await
            .expect("client side closed");
        let reply = read_reply(&mut pair.recv).await;
        (pair, reply)
    }

    /// Waits until the exit registers the client (registration happens after `EXIT_CODE_OK`).
    async fn wait_client_listed(&self) {
        while !self.cfg.clients.active_peers_at(Instant::now()).contains(&pid(1)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// End of `handle`'s work (its future returned).
    async fn handle_ended(&self) {
        self.ended.acquire().await.expect("semaphore alive").forget();
    }
}

async fn setup(opts: Opts) -> Fx {
    let site = spawn_site(opts.site).await;
    let cfg = exit_cfg(site);
    cfg.enabled.store(opts.enabled, Ordering::Relaxed);
    if opts.member {
        cfg.members.add_verified_conn(pid(1));
    }
    let ended = Arc::new(Semaphore::new(0));
    let handler = Arc::new(Tracked {
        inner: exit_service(cfg.clone()),
        ended: ended.clone(),
    });
    let net = MemNet::new();
    let server = NetStackBuilder::new(secret(2))
        .transport(Spy::on(&net, &secret(2)).shared())
        .serve(EXIT, handler)
        .build()
        .expect("exit built");
    let client = NetStackBuilder::new(secret(1))
        .transport(Spy::on(&net, &secret(1)).shared())
        .build()
        .expect("client built");
    client.add_hints(server.invite_addrs()[0].clone());
    let conn = within("connect", client.connect(pid(2), EXIT))
        .await
        .expect("connection");
    Fx {
        cfg,
        conn,
        ended,
        _stacks: (client, server),
    }
}

/// Exit settings: the domain `site.test` leads to `site`.
fn exit_cfg(site: SocketAddr) -> ExitConfig {
    let resolver = Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a stub address");
    ExitConfig {
        enabled: Arc::new(AtomicBool::new(true)),
        members: Members::default(),
        list: Arc::new(OnceLock::new()),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(HashMap::from([("site.test".to_string(), site)])),
        clients: Clients::default(),
        traffic: ExitTraffic::default(),
    }
}

/// A local "site": one accepted connection.
async fn spawn_site(kind: Site) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("site listener");
    let addr = listener.local_addr().expect("site address");
    tokio::spawn(async move {
        let Ok((mut tcp, _)) = listener.accept().await else {
            return;
        };
        match kind {
            Site::Echo => {
                let (mut r, mut w) = tcp.split();
                let _ = tokio::io::copy(&mut r, &mut w).await;
            }
            Site::Silent => std::future::pending::<()>().await,
        }
    });
    addr
}

/// A handler wrapper: raises a semaphore when `handle` returned.
struct Tracked {
    inner: Arc<dyn ServiceHandler>,
    ended: Arc<Semaphore>,
}

impl ServiceHandler for Tracked {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let (run, ended) = (self.inner.handle(conn), self.ended.clone());
        Box::pin(async move {
            run.await;
            ended.add_permits(1);
        })
    }
}

/// A request frame `[EXIT_PROTOCOL_VERSION, name length, name, port]` — the same one `ask_exit_to_connect` sends.
fn request(domain: &str, port: u16) -> Vec<u8> {
    encode_exit_request(domain, port).expect("the frame encodes")
}

/// Two reply bytes (up to `STEP`).
async fn read_reply(recv: &mut BoxRecv) -> [u8; 2] {
    let mut reply = [0u8; 2];
    within("exit reply", recv.read_exact(&mut reply))
        .await
        .expect("reply arrived");
    reply
}
