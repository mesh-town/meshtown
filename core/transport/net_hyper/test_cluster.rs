//! Test DHT network on one address (loopback by default): several nodes, the first one is the bootstrap for the rest.
//! Modeled on the cluster from the peeroxide-dht fork tests (host = 127.0.0.1, firewalled = false).
use libudx::UdxRuntime;
use peeroxide_dht::holepuncher::Holepuncher;
use peeroxide_dht::hyperdht::{spawn, HyperDhtConfig, HyperDhtError, HyperDhtHandle, ServerEvent};
use peeroxide_dht::rpc::DhtConfig;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::{Semaphore, SemaphorePermit};
use tokio::task::JoinHandle;

/// Clusters on a LAN address that may live at once in one test process: with more of them, dials between their
/// nodes run past the test timeouts. Loopback clusters are not limited.
const LAN_CLUSTERS_AT_ONCE: usize = 2;
static LAN_CLUSTERS: Semaphore = Semaphore::const_new(LAN_CLUSTERS_AT_ONCE);

/// One node of the cluster; the fields keep it alive while `Cluster` is alive.
struct Node {
    _rt: UdxRuntime,
    _handle: HyperDhtHandle,
    _events: UnboundedReceiver<ServerEvent>,
    _task: JoinHandle<Result<(), HyperDhtError>>,
}

/// A cluster of routing nodes sharing one address; lives until dropped.
pub struct Cluster {
    bootstrap: Vec<String>,
    _nodes: Vec<Node>,
    _lan_slot: Option<SemaphorePermit<'static>>,
}

/// A place among the LAN clusters, held until the cluster is dropped; `None` for loopback.
async fn lan_slot(host: &str) -> Option<SemaphorePermit<'static>> {
    if host.starts_with("127.") {
        return None;
    }
    Some(
        LAN_CLUSTERS
            .acquire()
            .await
            .expect("the semaphore is never closed"),
    )
}

impl Cluster {
    /// Bootstrap addresses of the form `<host>:<port>` — for `PeeroxideFactory { bootstrap }`.
    pub fn bootstrap(&self) -> Vec<String> {
        self.bootstrap.clone()
    }
}

async fn spawn_node(host: &str, bootstrap: Vec<String>) -> (Node, u16) {
    let rt = UdxRuntime::new().expect("udx runtime");
    let mut dht = DhtConfig::default();
    dht.bootstrap = bootstrap;
    dht.host = host.to_string();
    dht.firewalled = false;
    dht.ephemeral = Some(false);
    let mut config = HyperDhtConfig::default();
    config.dht = dht;
    let (task, handle, events) = spawn(&rt, config).await.expect("node start");
    handle.bootstrapped().await.expect("node warmed up");
    let port = handle.local_port().await.expect("node port");
    let node = Node {
        _rt: rt,
        _handle: handle,
        _events: events,
        _task: task,
    };
    (node, port)
}

/// Starts `size` nodes (at least one) on loopback; the first one is the bootstrap for the rest.
pub async fn cluster(size: usize) -> Cluster {
    cluster_on("127.0.0.1", size).await
}

/// Starts `size` nodes (at least one) on `host`. All nodes of a test share one address: a UDP socket
/// bound to a LAN address cannot send to loopback on some systems, and the other way round.
pub async fn cluster_on(host: &str, size: usize) -> Cluster {
    let lan_slot = lan_slot(host).await;
    let (first, port) = spawn_node(host, vec![]).await;
    let bootstrap = vec![format!("{host}:{port}")];
    let mut nodes = vec![first];
    for _ in 1..size {
        nodes.push(spawn_node(host, bootstrap.clone()).await.0);
    }
    Cluster {
        bootstrap,
        _nodes: nodes,
        _lan_slot: lan_slot,
    }
}

/// The first non-loopback IPv4 address of this machine: a relay there is reachable by address.
pub(crate) fn lan_address() -> String {
    Holepuncher::local_addresses(0)
        .into_iter()
        .map(|a| a.host)
        .find(|h| !h.starts_with("127."))
        .expect("a non-loopback IPv4 address")
}
