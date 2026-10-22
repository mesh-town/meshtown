use super::*;
use crate::network_key::NetworkKey;
use tokio::time::{timeout, Instant};

/// After `shutdown` all background tasks started in `start_node` must abort —
/// not hang around appending to the log after a network change/shutdown.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_aborts_background_tasks() {
    let key = NetworkKey::generate().unwrap();
    let node = start_node(NodeConfig::for_loopback_test(key)).await.unwrap();
    let tasks = node.background_abort_handles.clone();
    assert!(
        !tasks.is_empty(),
        "background tasks must exist in this configuration"
    );
    node.shutdown().await;

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if tasks.iter().all(|h| h.is_finished()) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "not all background tasks finished within 1 s after shutdown"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// assumption: `RunningNode` has `pub stack: NetStack`; `NetStack::{me, is_closed, snapshot}` exist in `net/facade/stack.rs`.

/// Node shutdown on the stack: `shutdown` closes the stack, including
/// with two holders.
mod stopped_node {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_closes_stack_and_endpoint() {
        // given: a loopback node and its clone
        let node = loopback_node().await;
        let clone = node.clone();
        // when: the owner's shutdown
        node.shutdown().await;
        // then: the clone's stack is closed
        assert!(
            clone.stack.is_closed(),
            "shutdown did not close the stack (stack.close)"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_from_two_clones_returns() {
        // given: a node and its clone
        let node = loopback_node().await;
        let stack = node.stack.clone();
        // when: two shutdowns at once
        let returned = shutdown_together(node.clone(), node).await;
        // then: both returned in time (no mutual blocking or panic), the stack is closed
        assert!(
            returned,
            "two shutdowns did not return within {BOTH_SHUTDOWNS_LIMIT:?}"
        );
        assert!(stack.is_closed(), "the stack is not closed after two shutdowns");
    }
}

/// The makeup of a freshly started node: the stack is consistent with the identity and telemetry.
mod started_node {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stack_matches_node_identity() {
        // given: a loopback node
        let node = loopback_node().await;
        // then: the stack is up under the node's key, with a single mem transport
        assert_eq!(node.stack.me(), node.id, "NetStackBuilder took a foreign key");
        assert_eq!(node.stack.snapshot().transport_names, ["mem"]);
        node.shutdown().await;
    }
}

// assumption: `RunningNode.background_abort_handles: Arc<Vec<AbortHandle>>` (the field is visible from the test — it is already read by
// `shutdown_aborts_background_tasks`); `NodeConfig.geoip: bool`;


/// Background tasks that depend on settings: joining the DHT lives in the stack, the country
/// poll depends on `geoip`. Every `AbortHandle` must land in `boot.background_abort_handles`, otherwise that guarantee won't hold.
mod background_tasks {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn start_registers_dht_and_geo_tasks() {
        // given: a node without a geo check and a node with a geo check
        let plain = setup(Opts::default()).await;
        let with_both = setup(Opts {
            geoip: true,
        })
        .await;
        let (plain_tasks, both_tasks) = (plain.background_abort_handles.len(), with_both.background_abort_handles.len());
        let handles = with_both.background_abort_handles.clone();
        // when: both are up
        // then: the second has exactly one more task — the country poll; the DHT lives in the stack (6b), not in `background_abort_handles`
        assert_eq!(
            both_tasks,
            plain_tasks + 1,
            "with geoip there must be exactly one extra task (the country poll), the DHT is in the stack"
        );
        // "hyperdht" without a hyper transport entry is skipped: a loopback node has no discovery sources
        assert!(with_both.stack.snapshot().discovery_names.is_empty());
        // then: after shutdown all of the second node's handles finish within 1 s
        plain.shutdown().await;
        with_both.shutdown().await;
        assert!(
            all_finished_within(&handles, Duration::from_secs(1)).await,
            "not all tasks (including the DHT and the country poll) finished within 1 s after shutdown"
        );
    }
}

/// The ceiling for two simultaneous `shutdown` calls (closing the transports plus a margin).
const BOTH_SHUTDOWNS_LIMIT: Duration = Duration::from_secs(15);

/// Node preconditions for the background task tests: what is enabled on top of `for_loopback_test`.
#[derive(Default)]
struct Opts {
    /// `geoip = true`: the country poll starts.
    geoip: bool,
}

/// A loopback node with a new network key and settings from `opts`.
async fn setup(opts: Opts) -> RunningNode {
    let key = NetworkKey::generate().unwrap();
    let cfg = NodeConfig {
        geoip: opts.geoip,
        ..NodeConfig::for_loopback_test(key)
    };
    start_node(cfg).await.unwrap()
}

/// Waits until all handles finish; `false` — did not make it within `limit`.
async fn all_finished_within(tasks: &[tokio::task::AbortHandle], limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if tasks.iter().all(|h| h.is_finished()) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tasks.iter().all(|h| h.is_finished())
}


/// A loopback node without relay and DHT, with a new network key.
async fn loopback_node() -> RunningNode {
    let key = NetworkKey::generate().unwrap();
    start_node(NodeConfig::for_loopback_test(key)).await.unwrap()
}

/// Calls `shutdown` on two holders at once; `true` — both returned in time.
async fn shutdown_together(a: RunningNode, b: RunningNode) -> bool {
    let both = async { tokio::join!(a.shutdown(), b.shutdown()) };
    timeout(BOTH_SHUTDOWNS_LIMIT, both).await.is_ok()
}

/// Discovery in the node's stack (6b): the order of sources and the list per the config.
mod discovery_wiring {
    use super::*;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stack_lists_sources_from_config() {
        // a node with an invite writes the "invite" summary into the shared log — the source line tests wait their turn
        let _log = crate::node::discovery_sources::tests::LOG_LOCK.lock().await;
        let key = NetworkKey::generate().unwrap();
        let bare = start_node(NodeConfig::for_loopback_test(key.clone())).await.unwrap();
        assert!(bare.stack.snapshot().discovery_names.is_empty());
        bare.shutdown().await;

        let mut cfg = NodeConfig::for_loopback_test(key);
        let other = crate::net::facade::test_support::pid(70);
        cfg.initial_peers = vec![crate::net::PeerAddr::new(other)];
        let node = start_node(cfg).await.unwrap();
        assert_eq!(node.stack.snapshot().discovery_names, ["invite"]);
        node.shutdown().await;
    }
}

/// The stack close seam on an error in the phases after the network (`close_on_err`).
mod close_on_err_seam {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn phase_error_closes_stack_and_returns_error() {
        // given: a loopback node
        let node = loopback_node().await;
        // when: the phase fails
        let err = close_on_err(&node.stack, async {
            Err::<(), _>(anyhow::anyhow!("phase failed"))
        })
        .await
        .unwrap_err();
        // then: the same error, the stack is closed
        assert_eq!(err.to_string(), "phase failed");
        assert!(node.stack.is_closed(), "a phase error did not close the stack");
        node.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn phase_ok_keeps_stack_open() {
        // given: a loopback node
        let node = loopback_node().await;
        // when: the phase returned Ok(7)
        let done = close_on_err(&node.stack, async { Ok(7) }).await.unwrap();
        // then: the same value, the stack is not closed
        assert_eq!(done, 7);
        assert!(!node.stack.is_closed(), "a successful phase closed the stack");
        node.shutdown().await;
    }
}

/// The SOCKS5 port is free by the time `shutdown` returns.
mod socks_port_release {
    use super::*;
    use crate::node::socks_task::tests::free_port;

    fn socks_cfg(port: u16) -> NodeConfig {
        let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
        cfg.socks.enabled = true;
        cfg.socks.port = port;
        cfg
    }

    fn can_bind(port: u16) -> bool {
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    const BUSY: &str = "for the browser proxy is in use by another program";

    #[tokio::test]
    async fn shutdown_frees_socks_port_before_return() {
        // given: a node is up on a fixed port
        let port = free_port();
        let node = start_node(socks_cfg(port)).await.unwrap();
        // when: shutdown
        node.shutdown().await;
        // then: the port is free at once
        assert!(can_bind(port), "port {port} is busy after shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn twenty_restarts_on_one_port() {
        let port = free_port();
        for round in 0..20 {
            let node = start_node(socks_cfg(port)).await.unwrap_or_else(|e| {
                panic!("round {round}, port {port}: {e:#}");
            });
            assert_eq!(node.socks_addr.unwrap().port(), port, "round {round}");
            node.shutdown().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn foreign_listener_fails_start_with_same_text() {
        let foreign = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = foreign.local_addr().unwrap().port();
        let err = start_node(socks_cfg(port)).await.err().expect("start on a busy port");
        let text = format!("{err:#}");
        assert!(text.contains(&format!("port {port} {BUSY}")), "port {port}: {text}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn foreign_listener_after_shutdown_still_fails() {
        let port = free_port();
        let node = start_node(socks_cfg(port)).await.unwrap();
        node.shutdown().await;
        let _foreign = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
        let err = start_node(socks_cfg(port)).await.err().expect("start on a busy port");
        let text = format!("{err:#}");
        assert!(text.contains(&format!("port {port} {BUSY}")), "port {port}: {text}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn open_client_does_not_hold_port() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let port = free_port();
        let node = start_node(socks_cfg(port)).await.unwrap();
        let addr = node.socks_addr.unwrap();
        let mut client = tokio::net::TcpStream::connect(addr).await.unwrap();
        client.write_all(&[5, 1, 0]).await.unwrap();
        let mut reply = [0u8; 2];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply, [5, 0], "port {port}");
        node.shutdown().await;
        assert!(can_bind(port), "port {port} is busy while a client is open");
    }
}
