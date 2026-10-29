//! What the relay support of townjump promises and the dialer relies on: when the direct path is closed,
//! a connection through a relay node still carries data, and the relay sees the pairing.
//!
//! The nodes use the fork API of the DHT directly, because the public wrapper does not expose the
//! knobs that close the direct path (the punch filter, `local_connection`).

use super::test_cluster::{cluster_on, lan_address};
use libudx::UdxRuntime;
use peeroxide_dht::blind_relay::BlindRelayServer;
use peeroxide_dht::crypto::hash;
use peeroxide_dht::hyperdht::{
    spawn, ConnectOpts, HolepunchFilter, HyperDhtConfig, HyperDhtHandle, KeyPair, PeerConnection,
    RelayPath, RelayThrough, ServerEvent,
};
use peeroxide_dht::relay_service::{run_relay_server, RelayServiceConfig};
use peeroxide_dht::rpc::DhtConfig;
use peeroxide_dht::server::{Server, ServerOpts};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

const LIMIT: Duration = Duration::from_secs(60);
const PAYLOAD_LEN: usize = 4096;

mod relay_contract {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn closed_direct_path_carries_data_through_the_relay() {
        // given: a relay, a firewalled echo server, and a firewalled client that vetoes every punch, all on one LAN address
        // (a socket bound to a LAN address cannot send to loopback on some systems, and the other way round)
        let lan = lan_address();
        let cl = cluster_on(&lan, 4).await;
        let relay = start_relay(cl.bootstrap(), &lan).await;
        let server = start_echo_server(cl.bootstrap(), &lan).await;
        let client = start_node(cl.bootstrap(), &lan, true, true).await;
        wait_until_findable(&relay.key.public_key, cl.bootstrap(), &lan).await;
        // when: the client connects to the server through the relay and sends a payload
        let mut conn = connect_through(&client, &server.key, relay.key.public_key).await;
        let sent: Vec<u8> = (0..PAYLOAD_LEN).map(|i| (i % 251) as u8).collect();
        conn.stream.write(&sent).await.expect("payload sent");
        let echoed = read_exactly(&mut conn, PAYLOAD_LEN).await;
        // then: the echo is intact, the connection is marked relayed, and the relay paired exactly one session
        assert_eq!(echoed, sent);
        let path = conn.relay_path().map(|rx| *rx.borrow());
        assert_eq!(path, Some(RelayPath::Relayed));
        assert_eq!(relay.server.stats().pairings_matched, 1);
    }
}

/// A DHT node of the fork, kept alive with its runtime.
struct Node {
    rt: UdxRuntime,
    handle: HyperDhtHandle,
    events: UnboundedReceiver<ServerEvent>,
}

struct Relay {
    key: KeyPair,
    server: BlindRelayServer,
    _rt: UdxRuntime,
    _task: peeroxide_dht::relay_service::RelayServiceHandle,
}

struct EchoServer {
    key: KeyPair,
    _server: Server,
    _node: (UdxRuntime, HyperDhtHandle),
}

async fn start_node(bootstrap: Vec<String>, host: &str, firewalled: bool, ephemeral: bool) -> Node {
    let rt = UdxRuntime::new().expect("UDX runtime");
    let mut dht = DhtConfig::default();
    dht.bootstrap = bootstrap;
    dht.host = host.to_string();
    dht.firewalled = firewalled;
    dht.ephemeral = Some(ephemeral);
    let mut config = HyperDhtConfig::default();
    config.dht = dht;
    let (_task, handle, events) = spawn(&rt, config).await.expect("node start");
    handle.bootstrapped().await.expect("node warmed up");
    Node { rt, handle, events }
}

async fn start_relay(bootstrap: Vec<String>, host: &str) -> Relay {
    let node = start_node(bootstrap, host, false, false).await;
    let key = KeyPair::from_seed([0x71; 32]);
    let (server, task) = run_relay_server(
        node.rt.handle(),
        node.handle.clone(),
        key.clone(),
        node.events,
        RelayServiceConfig::default(),
    );
    Relay {
        key,
        server,
        _rt: node.rt,
        _task: task,
    }
}

/// A firewalled server that echoes everything of its first connection.
async fn start_echo_server(bootstrap: Vec<String>, host: &str) -> EchoServer {
    let node = start_node(bootstrap, host, true, true).await;
    let key = KeyPair::from_seed([0x72; 32]);
    let shared = UdxRuntime::shared(node.rt.handle());
    let (server, mut incoming) = Server::new(
        node.handle.clone(),
        node.events,
        shared,
        ServerOpts::default(),
    );
    server.listen(key.clone()).await.expect("server listens");
    crate::transport::net::task_label::spawn(async move {
        let mut conn = incoming.recv().await.expect("an incoming connection");
        while let Ok(Some(data)) = conn.stream.read().await {
            if conn.stream.write(&data).await.is_err() {
                return;
            }
        }
    });
    EchoServer {
        key,
        _server: server,
        _node: (node.rt, node.handle),
    }
}

/// Waits until the DHT finds the relay by its key: before that nobody can connect through it.
async fn wait_until_findable(relay_key: &[u8; 32], bootstrap: Vec<String>, host: &str) {
    let looker = start_node(bootstrap, host, true, false).await;
    for _ in 0..50 {
        if matches!(looker.handle.find_peer(hash(relay_key)).await, Ok(Some(_))) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("the relay was not found in the DHT");
}

/// Connects with the direct path closed: no local shortcut, every punch vetoed, the relay named.
async fn connect_through(client: &Node, server: &KeyPair, relay_key: [u8; 32]) -> PeerConnection {
    let mut opts = ConnectOpts::default();
    opts.local_connection = false;
    opts.relay_through = Some(RelayThrough::Key(relay_key));
    opts.holepunch = Some(HolepunchFilter::new(|_| false));
    let me = KeyPair::from_seed([0x73; 32]);
    let connecting =
        client
            .handle
            .connect_with_options(&me, server.public_key, &[], &client.rt, opts);
    tokio::time::timeout(LIMIT, connecting)
        .await
        .expect("the connection came up in time")
        .expect("connected")
}

async fn read_exactly(conn: &mut PeerConnection, len: usize) -> Vec<u8> {
    let mut got = Vec::new();
    while got.len() < len {
        let chunk = tokio::time::timeout(LIMIT, conn.stream.read())
            .await
            .expect("the echo arrived in time")
            .expect("read")
            .expect("the stream is still open");
        got.extend_from_slice(&chunk);
    }
    got
}
