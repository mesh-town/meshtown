//! Contract suites of `core/net/contract/` on `mem`: one line per suite and an environment on `MemNet`
//! (sync — on `MemSyncNet`). Each test takes a fresh environment: the networks of different tests do not overlap.

use crate::net::addr::{PeerAddr, RelayRef};
use crate::net::connect::{Connector, PacketPath, PathInfo};
use crate::net::contract::connector::connector_contract;
use crate::net::contract::discovery::discovery_contract;
use crate::net::contract::membership::membership_contract;
use crate::net::contract::replica::replica_contract;
use crate::net::contract::transport::transport_contract;
use crate::net::contract::transport_diag::transport_diag_contract;
use crate::net::contract::{
    pid, secret, ConnectorHarness, DiscoveryHarness, Harness, RawPut, SyncPair,
};
use crate::net::discovery::Discovery;
use crate::net::error::NetResult;
use crate::net::link::NetInfo;
use crate::net::mem::{MemCarrier, MemConnector, MemNet, MemSyncNet};
use crate::net::service::{AUTH, PORT};
use crate::net::split::SplitTransport;
use crate::net::stream::BoxFuture;
use crate::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::net::transport::Transport;
use bytes::Bytes;
use std::any::Any;
use std::io;
use std::sync::Arc;

connector_contract!(connector_harness);
discovery_contract!(discovery_harness);
transport_contract!(transport_harness);
transport_diag_contract!(transport_harness);
replica_contract!(mem_sync_harness);
membership_contract!(mem_sync_harness);

mod carrier {
    use super::*;
    use crate::net::contract::{must_finish, SEED_A, SEED_B};
    use crate::net::error::NetError;
    use crate::net::transport::PeerConn;

    #[tokio::test]
    async fn path_of_a_foreign_family_has_no_route() {
        // given: the connector of A hands out paths with family = "other", B is ordinary
        let net = MemNet::new();
        let a = foreign_transport(&net, SEED_A);
        let b = net.transport(&secret(SEED_B));
        b.serve(&[AUTH, PORT]);
        // when: A connects to B
        let result = must_finish(a.connect(&b.local_addr(), AUTH)).await;
        // then: NoRoute (the mem carrier does not take a foreign path)
        expect_no_route(result);
    }

    fn expect_no_route(result: NetResult<Arc<dyn PeerConn>>) {
        match result {
            Err(NetError::NoRoute) => {}
            Err(other) => panic!("expected NoRoute, got the error: {other}"),
            Ok(_) => panic!("expected NoRoute, got a connection"),
        }
    }
}

/// Environment of the connection suite: one `MemNet` per test, a node is `net.connector(&secret(seed))`.
struct MemConnectorHarness {
    net: Arc<MemNet>,
}

impl MemConnectorHarness {
    fn connector(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Connector>> {
        let connector: Arc<dyn Connector> = self.net.connector(&secret(seed));
        Box::pin(async move { connector })
    }
}

impl ConnectorHarness for MemConnectorHarness {
    fn node(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Connector>> {
        self.connector(seed)
    }

    // A restart in `mem` is a new slot of the same key: `MemNet::connector` closes the old one.
    fn restart(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Connector>> {
        self.connector(seed)
    }
}

/// Environment of the discovery suite: sources with one name share one board on a common `MemNet`.
struct MemDiscoveryHarness {
    net: Arc<MemNet>,
}

impl DiscoveryHarness for MemDiscoveryHarness {
    fn source(&self, publishes: bool) -> Arc<dyn Discovery> {
        self.net.discovery("mem", publishes)
    }
}

/// Environment of the transport suite: one `MemNet` per test, a node is `net.transport(&secret(seed))` with `serve([AUTH, PORT])`.
struct MemTransportHarness {
    net: Arc<MemNet>,
}

impl MemTransportHarness {
    fn transport(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Transport>> {
        let transport = self.net.transport(&secret(seed));
        transport.serve(&[AUTH, PORT]);
        let transport: Arc<dyn Transport> = transport;
        Box::pin(async move { transport })
    }
}

impl Harness for MemTransportHarness {
    fn node(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Transport>> {
        self.transport(seed)
    }

    // A restart in `mem` is a new slot of the same key: `MemNet::connector` closes the old one.
    fn restart(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Transport>> {
        self.transport(seed)
    }
}

fn connector_harness() -> MemConnectorHarness {
    MemConnectorHarness { net: MemNet::new() }
}

fn discovery_harness() -> MemDiscoveryHarness {
    MemDiscoveryHarness { net: MemNet::new() }
}

fn transport_harness() -> MemTransportHarness {
    MemTransportHarness { net: MemNet::new() }
}

/// Environment of the sync suites: a fresh `MemSyncNet`, nodes `a` and `b` with keys `SEED_A`/`SEED_B` in one
/// network (`network_seed` `[9; 32]`), in-memory storage. `raw` puts a foreign entry into "net" of node `a`.
async fn mem_sync_harness() -> SyncPair {
    use crate::net::contract::{SEED_A, SEED_B};
    let net = MemSyncNet::new();
    let ctx = |seed: u8| SyncCtx {
        secret: secret(seed),
        network_seed: [9; 32],
        store_dir: None,
    };
    let (a, b) = (net.backend(ctx(SEED_A)), net.backend(ctx(SEED_B)));
    let raw_node = a.clone();
    let raw: RawPut = Arc::new(move |entry| {
        let replica = raw_node.replica_mem(REPLICA_NET);
        replica.expect("replica \"net\" of node a").put_raw(entry);
    });
    SyncPair {
        a,
        b,
        _keep: Box::new(net),
        raw: Some(raw),
    }
}

/// A `mem` transport whose connection-layer path passes itself off as a path of another family.
fn foreign_transport(net: &MemNet, seed: u8) -> SplitTransport {
    let inner = net.connector(&secret(seed));
    let connector = Arc::new(ForeignConnector { inner });
    SplitTransport::new(pid(seed), connector, Arc::new(MemCarrier::new()))
}

/// A `mem` connector whose paths (dialed and accepted) call themselves the family `"other"`.
struct ForeignConnector {
    inner: Arc<MemConnector>,
}

fn foreign(path: Box<dyn PacketPath>) -> Box<dyn PacketPath> {
    Box::new(ForeignPath { inner: path })
}

impl Connector for ForeignConnector {
    fn kind(&self) -> &'static str {
        self.inner.kind()
    }

    fn local_addr(&self) -> PeerAddr {
        self.inner.local_addr()
    }

    fn can_reach(&self, to: &PeerAddr) -> bool {
        self.inner.can_reach(to)
    }

    fn add_hints(&self, addr: &PeerAddr) {
        self.inner.add_hints(addr);
    }

    fn set_relays(&self, relays: &[RelayRef]) {
        self.inner.set_relays(relays);
    }

    fn dial(&self, to: &PeerAddr) -> BoxFuture<'_, NetResult<Box<dyn PacketPath>>> {
        let to = to.clone();
        Box::pin(async move { Ok(foreign(self.inner.dial(&to).await?)) })
    }

    fn listen(&self) -> BoxFuture<'_, Option<Box<dyn PacketPath>>> {
        Box::pin(async move { self.inner.listen().await.map(foreign) })
    }

    fn net_info(&self) -> NetInfo {
        self.inner.net_info()
    }

    fn close(&self) -> BoxFuture<'_, ()> {
        self.inner.close()
    }
}

/// A `mem` path whose `info().family` is replaced with `"other"`.
struct ForeignPath {
    inner: Box<dyn PacketPath>,
}

impl PacketPath for ForeignPath {
    fn info(&self) -> PathInfo {
        PathInfo {
            family: "other",
            ..self.inner.info()
        }
    }

    fn max_packet_bytes(&self) -> usize {
        self.inner.max_packet_bytes()
    }

    fn send(&self, pkt: Bytes) -> BoxFuture<'_, io::Result<()>> {
        self.inner.send(pkt)
    }

    fn recv(&self) -> BoxFuture<'_, io::Result<Bytes>> {
        self.inner.recv()
    }

    fn close(&self) {
        self.inner.close();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
