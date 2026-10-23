use super::*;
use crate::members::network_key::NetworkKey;
use crate::config::config_file::ProcessSettings;
use crate::proxy::{ProxyFace, ProxyFront};
use tokio::time::{timeout, Instant};

/// After `shutdown` all background tasks started in `start_node` must abort —
/// not hang around appending to the log after a network change/shutdown.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_aborts_background_tasks() {
    let key = NetworkKey::generate().unwrap();
    let node = start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
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
        assert!(
            stack.is_closed(),
            "the stack is not closed after two shutdowns"
        );
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
        assert_eq!(
            node.stack.me(),
            node.id,
            "NetStackBuilder took a foreign key"
        );
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
        let with_both = setup(Opts { geoip: true }).await;
        let (plain_tasks, both_tasks) = (
            plain.background_abort_handles.len(),
            with_both.background_abort_handles.len(),
        );
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
    start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap()
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
        let bare = start_node(NodeConfig::for_loopback_test(key.clone()))
            .await
            .unwrap();
        assert!(bare.stack.snapshot().discovery_names.is_empty());
        bare.shutdown().await;

        let mut cfg = NodeConfig::for_loopback_test(key);
        let other = crate::transport::net::facade::test_support::pid(70);
        cfg.initial_peers = vec![crate::transport::net::PeerAddr::new(other)];
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
        assert!(
            node.stack.is_closed(),
            "a phase error did not close the stack"
        );
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
        assert!(
            !node.stack.is_closed(),
            "a successful phase closed the stack"
        );
        node.shutdown().await;
    }
}

/// The node never takes the proxy port: the process-wide front owns it.
mod proxy_port_held_by_a_front {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_nodes_start_while_the_proxy_port_is_held() {
        // given: a front holds the port and both configs name that same port
        let port = free_port();
        let front = ProxyFront::bind(&settings(port)).await.unwrap();
        let (first, second) = (cfg_with_port(port), cfg_with_port(port));

        // when: two nodes start
        let first = start_node(first).await;
        let second = start_node(second).await;

        // then: both start, and the front still holds the port
        assert!(first.is_ok(), "first node: {:?}", first.as_ref().err());
        assert!(second.is_ok(), "second node: {:?}", second.as_ref().err());
        assert_eq!(front.addr(), Some(loopback(port)));
        for node in [first, second].into_iter().flatten() {
            node.shutdown().await;
        }
    }
}

/// The proxy face a node reads: the supplied one, or a constant "off".
mod proxy_face {
    use super::*;

    mod not_supplied {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_off_though_the_config_enables_socks() {
            // given: a config that still enables the old listener settings, without a face
            let mut cfg = cfg_with_port(free_port());
            cfg.proxy_face = None;

            // when: the node starts
            let node = start_node(cfg).await.unwrap();

            // then: its face is the constant "off"
            assert_eq!(*node.proxy_face.borrow(), ProxyFace::OFF);
            node.shutdown().await;
        }
    }

    mod supplied {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn is_the_one_that_was_passed() {
            // given: a front on a port and a config with its face
            let port = free_port();
            let front = ProxyFront::bind(&settings(port)).await.unwrap();
            let mut cfg = cfg_with_port(0);
            cfg.proxy_face = Some(front.face());

            // when: the node starts
            let node = start_node(cfg).await.unwrap();

            // then: it sees the front's address
            assert_eq!(node.proxy_face.borrow().addr, Some(loopback(port)));
            node.shutdown().await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn follows_the_front_when_its_port_changes() {
            // given: a node reading the face of a front
            let front = ProxyFront::bind(&settings(free_port())).await.unwrap();
            let mut cfg = cfg_with_port(0);
            cfg.proxy_face = Some(front.face());
            let node = start_node(cfg).await.unwrap();

            // when: the front moves to another port
            let moved = free_port();
            front.apply(&settings(moved)).await.unwrap();

            // then: the node sees the new address
            assert_eq!(node.proxy_face.borrow().addr, Some(loopback(moved)));
            node.shutdown().await;
        }
    }
}

mod proxy_config {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn repeated_calls_share_one_udp_semaphore() {
        // given: a started node
        let node = loopback_node().await;

        // when: the configuration is taken twice
        let (first, second) = (node.proxy_config(), node.proxy_config());

        // then: both carry the same semaphore for UDP associations
        assert!(
            Arc::ptr_eq(&first.udp_slots, &second.udp_slots),
            "each call made its own semaphore"
        );
        node.shutdown().await;
    }
}

/// Stopping a node does not touch the process-wide proxy listener.
mod shutdown_with_a_front {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn front_keeps_listening_and_refuses_without_a_network() {
        // given: a front serving through a node
        let port = free_port();
        let front = ProxyFront::bind(&settings(port)).await.unwrap();
        let node = loopback_node().await;
        front.set_target(Some(node.proxy_config()));

        // when: the node stops and the front loses its target
        node.shutdown().await;
        front.set_target(None);

        // then: the front still listens on the port and refuses with "no network"
        assert_eq!(front.addr(), Some(loopback(port)));
        let answer = http_connect(loopback(port)).await;
        assert!(answer.starts_with("HTTP/1.1 502"), "answer: {answer}");
        assert!(
            answer.contains("no network for the proxy"),
            "answer: {answer}"
        );
    }
}

/// A loopback-test config whose old listener settings name `port`.
fn cfg_with_port(port: u16) -> NodeConfig {
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    cfg.socks.enabled = true;
    cfg.socks.port = port;
    cfg
}

fn settings(port: u16) -> ProcessSettings {
    ProcessSettings {
        socks_port: port,
        ..ProcessSettings::default()
    }
}

fn loopback(port: u16) -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, 1], port))
}

/// A free port on loopback: bind with port zero, remember it, close.
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// HTTP `CONNECT` to some host; returns the whole answer (a refusal closes the connection).
async fn http_connect(proxy: std::net::SocketAddr) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let request = "CONNECT site.test:80 HTTP/1.1\r\nHost: site.test:80\r\n\r\n";
    let mut stream = tokio::net::TcpStream::connect(proxy).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = Vec::new();
    timeout(Duration::from_secs(5), stream.read_to_end(&mut answer))
        .await
        .expect("the proxy did not close the connection")
        .unwrap();
    String::from_utf8_lossy(&answer).into_owned()
}
