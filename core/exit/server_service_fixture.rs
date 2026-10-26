//! Fixture of the exit handler tests on the mem stack: a client with a connection to an exit, local sites, requests.

use super::*;
use crate::exit::frame::encode_exit_request;
use crate::exit::ExitLimits;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::facade::test_support::{pid, secret, within, Spy};
use crate::transport::net::mem::MemNet;
use crate::transport::net::service::EXIT;
use crate::transport::net::{NetStack, NetStackBuilder, StreamPair};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

/// Preconditions: who asks and what site it is.
pub(super) struct Opts {
    pub enabled: bool,
    pub member: bool,
    pub site: Site,
    pub limits: ExitLimits,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            enabled: true,
            member: true,
            site: Site::Silent,
            limits: ExitLimits::unlimited(),
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
    accepts: Arc<std::sync::atomic::AtomicUsize>,
    _stacks: (NetStack, NetStack),
    held: Option<crate::exit::limits::StreamSlot>,
}

impl Fx {
    /// Keeps a place of the exit's limiter taken until the fixture is dropped.
    pub(super) fn hold(&mut self, slot: crate::exit::limits::StreamSlot) {
        self.held = Some(slot);
    }

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
        pair.send.shutdown().await.expect("client side closed");
        let reply = read_reply(&mut pair.recv).await;
        (pair, reply)
    }

    /// A request as `ask`, but the reply is read up to 2 bytes or to the end of the stream (a short reply ends early).
    pub(super) async fn ask_raw(&self, frame: &[u8]) -> (StreamPair, Vec<u8>) {
        let mut pair = within("open", self.conn.open())
            .await
            .expect("stream opened");
        pair.send.write_all(frame).await.expect("frame sent");
        let mut reply = Vec::new();
        while reply.len() < 2 {
            let mut chunk = [0u8; 2];
            let n = within("exit reply", pair.recv.read(&mut chunk[..2 - reply.len()]))
                .await
                .expect("reply or end of stream");
            if n == 0 {
                break;
            }
            reply.extend_from_slice(&chunk[..n]);
        }
        (pair, reply)
    }

    /// How many TCP connections the local site has accepted so far.
    pub(super) fn site_accepts(&self) -> usize {
        self.accepts.load(Ordering::SeqCst)
    }

    /// Waits until the exit registers the client (registration happens after `EXIT_CODE_OK`).
    pub(super) async fn wait_client_listed(&self) {
        while !self
            .cfg
            .clients
            .active_peers_at(Instant::now())
            .contains(&pid(1))
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// End of `handle`'s work (its future returned).
    pub(super) async fn handle_ended(&self) {
        self.ended
            .acquire()
            .await
            .expect("semaphore alive")
            .forget();
    }
}

pub(super) async fn setup(opts: Opts) -> Fx {
    let (site, accepts) = spawn_site(opts.site).await;
    let cfg = exit_cfg(site, opts.limits.clone());
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
        accepts,
        _stacks: (client, server),
        held: None,
    }
}

/// Exit settings: the domain `site.test` leads to `site`.
fn exit_cfg(site: SocketAddr, limits: ExitLimits) -> ExitConfig {
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
        limits,
    }
}

/// A local "site": accepts any number of connections and counts them.
async fn spawn_site(kind: Site) -> (SocketAddr, Arc<std::sync::atomic::AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("site listener");
    let addr = listener.local_addr().expect("site address");
    let accepts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = accepts.clone();
    crate::node::output::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            crate::node::output::spawn(serve_site_connection(kind, tcp));
        }
    });
    (addr, accepts)
}

async fn serve_site_connection(kind: Site, mut tcp: tokio::net::TcpStream) {
    match kind {
        Site::Echo => {
            let (mut r, mut w) = tcp.split();
            let _ = tokio::io::copy(&mut r, &mut w).await;
        }
        Site::Silent => std::future::pending::<()>().await,
    }
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
