//! Fixture of the exit handler tests on the mem stack: a client with a connection to an exit, local sites, requests.

use super::*;

/// Preconditions: who asks and what site it is.
pub(super) struct Opts {
    pub enabled: bool,
    pub member: bool,
    pub site: Site,
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
pub(super) enum Site {
    /// Returns everything it received.
    Echo,
    /// Accepts a connection and stays silent.
    Silent,
}

/// A client (node 1) with an open connection to an exit (node 2).
pub(super) struct Fx {
    pub cfg: ExitConfig,
    pub conn: Arc<dyn PeerConn>,
    ended: Arc<Semaphore>,
    _stacks: (NetStack, NetStack),
}

impl Fx {
    /// A request: the whole frame, the reply is the first two bytes (up to `STEP`).
    pub(super) async fn ask(&self, frame: &[u8]) -> (StreamPair, [u8; 2]) {
        let mut pair = within("open", self.conn.open())
            .await
            .expect("stream opened");
        pair.send.write_all(frame).await.expect("frame sent");
        let reply = read_reply(&mut pair.recv).await;
        (pair, reply)
    }

    /// The same, but after writing the client closes its side (the frame is cut off by the end of the stream).
    pub(super) async fn ask_then_close(&self, frame: &[u8]) -> (StreamPair, [u8; 2]) {
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
    pub(super) async fn wait_client_listed(&self) {
        while !self.cfg.clients.active_peers_at(Instant::now()).contains(&pid(1)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// End of `handle`'s work (its future returned).
    pub(super) async fn handle_ended(&self) {
        self.ended.acquire().await.expect("semaphore alive").forget();
    }
}

pub(super) async fn setup(opts: Opts) -> Fx {
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
pub(super) fn request(domain: &str, port: u16) -> Vec<u8> {
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
