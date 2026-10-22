//! Tests for `open_through_exits` (`path.rs`): a busy exit (its `open()` does not finish within the pool timeout)
//! is skipped and the next candidate answers; a single busy one gives the refusal "exit is busy".
//! The client stack is built on `MemNet`; the client transport replaces the connection to the "busy" peer with a
//! wrapper whose `open()` never returns. The live exit answers `EXIT_CODE_OK`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::doh::Resolver;
use crate::exit::{PeerConnPool, EXIT_CODE_OK, EXIT_PROTOCOL_VERSION};
use crate::http_min::HttpLimits;
use crate::net::facade::test_support::{pid, secret, within, Spy};
use crate::net::mem::MemNet;
use crate::net::service::EXIT;
use crate::net::{
    BoxFuture, LinkInfo, NetInfo, NetResult, NetStack, NetStackBuilder, PeerAddr, PeerConn,
    RelayRef, ServiceHandler, ServiceId, StreamPair, Transport,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const BUSY_TEXT: &str = "refused: exit is busy (stream limit)";

mod skipping {
    use super::*;

    /// given: the first candidate is busy, the second is alive; when `open_through_exits`, then the route is the second exit.
    #[tokio::test]
    async fn through_exits_busy_exit_is_skipped() {
        let cfg = config(&[2]).await;

        let res = within(
            "open_through_exits",
            open_through_exits(&cfg, &[pid(2), pid(3)], "example.com", 443),
        )
        .await;

        let opened = res.ok().expect("the second exit answers");
        assert_eq!(opened.route_label, format!("exit {}", short_peer_id(&pid(3))));
    }

    /// given: the only candidate is busy; when `open_through_exits`, then a refusal with the reason "busy".
    #[tokio::test]
    async fn through_exits_only_busy_exit_is_refused_as_busy() {
        let cfg = config(&[2]).await;

        let res = within(
            "open_through_exits",
            open_through_exits(&cfg, &[pid(2)], "example.com", 443),
        )
        .await;

        let refused = res.err().expect("the exit is busy");
        assert_eq!(refused.outcome.route_label, "no route");
        assert_eq!(refused.outcome.result, BUSY_TEXT);
    }
}

mod remembered_gateway {
    use super::*;

    /// given: the exit is stuck but is remembered as refusing the gateway target; when the gateway target is
    /// requested, then the refusal comes without asking the exit (an ask would end as "busy").
    #[tokio::test]
    async fn remembered_refusal_is_not_asked_again() {
        let cfg = config(&[2]).await;
        cfg.pool.note_gateway_refused(pid(2));

        let res = within(
            "open_through_exits",
            open_through_exits(&cfg, &[pid(2)], exit::UDPGW_HOST, exit::UDPGW_PORT),
        )
        .await;

        let refused = res.err().expect("refused from memory");
        assert_eq!(refused.outcome.route_label, format!("exit {}", short_peer_id(&pid(2))));
        assert!(refused.outcome.result.contains("code 7"), "{}", refused.outcome.result);
    }

    /// given: the same remembered exit; when an ordinary target is requested, then the exit is still asked.
    #[tokio::test]
    async fn ordinary_target_is_not_affected_by_the_memory() {
        let cfg = config(&[2]).await;
        cfg.pool.note_gateway_refused(pid(2));

        let res = within("open_through_exits", open_through_exits(&cfg, &[pid(2)], "example.com", 443)).await;

        assert_eq!(res.err().expect("busy").outcome.result, BUSY_TEXT);
    }
}

/// A proxy config: the client (1), exits 2 and 3 (both answer `EXIT_CODE_OK`); peers from `stuck` are "busy".
async fn config(stuck: &[u8]) -> ProxyConfig {
    config_with(stuck, || Arc::new(AlwaysOk)).await
}

/// Like `config`, with the handler `exit` makes for each of the exits 2 and 3.
pub(super) async fn config_with(stuck: &[u8], exit: impl Fn() -> Arc<dyn ServiceHandler>) -> ProxyConfig {
    let net = MemNet::new();
    let client = client_stack(&net, stuck);
    for seed in [2u8, 3] {
        let server = exit_stack(&net, seed, exit());
        client.add_hints(server.invite_addrs()[0].clone());
        std::mem::forget(server);
    }
    let key = crate::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let resolver = Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a stub address");
    ProxyConfig {
        route: RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, vec![]),
        stack: client,
        pool: PeerConnPool::default().with_open_timeout(Duration::from_millis(100)),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(HashMap::new()),
        udp_slots: crate::proxy::udp_slots(),
    }
}

fn client_stack(net: &Arc<MemNet>, stuck: &[u8]) -> NetStack {
    let inner = Spy::on(net, &secret(1)).shared();
    let stuck = stuck.iter().map(|s| pid(*s)).collect();
    let transport = Arc::new(Stucking { inner, stuck });
    NetStackBuilder::new(secret(1))
        .transport(transport)
        .build()
        .expect("the client was built")
}

fn exit_stack(net: &Arc<MemNet>, seed: u8, handler: Arc<dyn ServiceHandler>) -> NetStack {
    NetStackBuilder::new(secret(seed))
        .transport(Spy::on(net, &secret(seed)).shared())
        .serve(EXIT, handler)
        .build()
        .expect("the exit was built")
}

/// A wrapper transport: connections to peers from `stuck` are returned with a hung `open()`.
struct Stucking {
    inner: Arc<dyn Transport>,
    stuck: Vec<crate::net::PeerId>,
}

impl Transport for Stucking {
    fn kind(&self) -> &'static str {
        self.inner.kind()
    }
    fn me(&self) -> crate::net::PeerId {
        self.inner.me()
    }
    fn local_addr(&self) -> PeerAddr {
        self.inner.local_addr()
    }
    fn can_reach(&self, to: &PeerAddr) -> bool {
        self.inner.can_reach(to)
    }
    fn add_hints(&self, addr: &PeerAddr) {
        self.inner.add_hints(addr)
    }
    fn set_relays(&self, relays: &[RelayRef]) {
        self.inner.set_relays(relays)
    }
    fn serve(&self, svcs: &[ServiceId]) {
        self.inner.serve(svcs)
    }
    fn connect(
        &self,
        to: &PeerAddr,
        svc: ServiceId,
    ) -> BoxFuture<'_, NetResult<Arc<dyn PeerConn>>> {
        let busy = self.stuck.contains(&to.id);
        let fut = self.inner.connect(to, svc);
        Box::pin(async move {
            let conn = fut.await?;
            if busy {
                return Ok(Arc::new(StuckConn { inner: conn }) as Arc<dyn PeerConn>);
            }
            Ok(conn)
        })
    }
    fn accept(&self) -> BoxFuture<'_, Option<(ServiceId, Arc<dyn PeerConn>)>> {
        self.inner.accept()
    }
    fn net_info(&self) -> NetInfo {
        self.inner.net_info()
    }
    fn close(&self) -> BoxFuture<'_, ()> {
        self.inner.close()
    }
}

/// A connection whose `open()` never returns; the rest is delegated to the real one.
struct StuckConn {
    inner: Arc<dyn PeerConn>,
}

impl PeerConn for StuckConn {
    fn peer(&self) -> crate::net::PeerId {
        self.inner.peer()
    }
    fn stable_id(&self) -> u64 {
        self.inner.stable_id()
    }
    fn open(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        Box::pin(std::future::pending())
    }
    fn accept(&self) -> BoxFuture<'_, NetResult<StreamPair>> {
        self.inner.accept()
    }
    fn link(&self) -> LinkInfo {
        self.inner.link()
    }
    fn close(&self, code: u32, reason: &[u8]) {
        self.inner.close(code, reason)
    }
    fn closed(&self) -> BoxFuture<'_, ()> {
        self.inner.closed()
    }
    fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

/// A live exit: reads the whole request frame and answers `EXIT_CODE_OK`.
struct AlwaysOk;

impl ServiceHandler for AlwaysOk {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            while let Ok(mut pair) = conn.accept().await {
                let mut head = [0u8; 2];
                if pair.recv.read_exact(&mut head).await.is_err() {
                    continue;
                }
                let mut tail = vec![0u8; usize::from(head[1]) + 2];
                let _ = pair.recv.read_exact(&mut tail).await;
                let _ = pair.send.write_all(&[EXIT_PROTOCOL_VERSION, EXIT_CODE_OK]).await;
                let _ = pair.send.shutdown().await;
            }
        })
    }
}
