use super::*;
use crate::members::network_key::NetworkKey;
use crate::node::discovery_sources::tests::LOG_LOCK;
use crate::node::output::log_page;
use crate::node::start::boot::boot_endpoint;
use crate::node::start::network::spawn_network;

#[test]
fn socks_off_line_names_flag() {
    assert_eq!(socks_off_line(true), "off by the --no-socks flag");
}

#[test]
fn socks_off_line_names_settings() {
    assert_eq!(
        socks_off_line(false),
        "off in settings (socks.enabled: false)"
    );
}

#[test]
fn exit_line_names_both_modes() {
    assert_eq!(
        exit_line(true),
        format!(
            "serving as exit for members of network {}",
            crate::node::output::NET_NAME
        )
    );
    assert_eq!(exit_line(false), "not serving as exit (--no-exit)");
}

#[test]
fn blocked_line_none_when_empty() {
    assert_eq!(blocked_line(&HashSet::new()), None);
}

#[test]
fn blocked_line_sorts_names() {
    let blocked: HashSet<String> = ["b.example", "a.example"]
        .iter()
        .map(|d| d.to_string())
        .collect();
    assert_eq!(
        blocked_line(&blocked).as_deref(),
        Some("treating as blocked: a.example, b.example")
    );
}

/// Brings up the phases up to `serving` and calls `setup_serving`; returns the SOCKS5 address and the `socks` lines.
async fn serve_with(socks_enabled: bool) -> (Option<SocketAddr>, Vec<String>) {
    let served = serve_full(socks_enabled).await;
    (served.addr, served.lines)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn setup_serving_without_socks() {
    // given: SOCKS5 is off in settings
    let (addr, lines) = serve_with(false).await;
    // then: no address, the reason is in the log
    assert_eq!(addr, None);
    assert!(
        lines
            .iter()
            .any(|l| l == "off in settings (socks.enabled: false)"),
        "{lines:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn setup_serving_with_socks_port_zero() {
    // given: the default (port 0 — a free one)
    let (addr, _) = serve_with(true).await;
    // then: there is a listener, the port taken is a real one
    assert_ne!(addr.expect("SOCKS5 address").port(), 0);
}

/// The proxy configuration the serving phase builds for a front: shared by every clone, with or without the
/// node's own listener.
mod proxy_config {
    use super::*;
    use crate::node::start_node;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_built_when_socks_is_off_in_settings() {
        // given: SOCKS5 is off in settings, so the node has no listener
        let served = serve_full(false).await;
        // then: the configuration is still there and its association limit is untouched
        assert_eq!(served.addr, None);
        assert_eq!(
            served.proxy.udp_slots.available_permits(),
            crate::proxy::udp_slots().available_permits(),
            "a fresh configuration must have all UDP association slots free"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clone_shares_udp_slots() {
        // given: a node with a listener
        let served = serve_full(true).await;
        // when: the configuration is cloned
        let clone = served.proxy.clone();
        // then: the clone holds the same semaphore, not a new one
        assert!(
            Arc::ptr_eq(&served.proxy.udp_slots, &clone.udp_slots),
            "a clone must share the UDP association limit"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clone_shares_the_connection_pool() {
        // given: a configuration and its clone
        let served = serve_full(false).await;
        let clone = served.proxy.clone();
        // when: the clone's pool notes a refusal of the UDP gateway target
        clone.pool.note_gateway_refused(served.id);
        // then: the original's pool sees it as well
        assert!(
            served.proxy.pool.gateway_refused_recently(served.id),
            "a clone must share the pool state with the original"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_nodes_have_separate_udp_slots() {
        // given: two nodes served one after another
        let (first, second) = (serve_full(false).await, serve_full(false).await);
        // then: each has its own association limit
        assert!(
            !Arc::ptr_eq(&first.proxy.udp_slots, &second.proxy.udp_slots),
            "nodes must not share one UDP association limit"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn running_node_hands_out_configs_with_the_same_udp_slots() {
        // given: a started node
        let node = start_node(NodeConfig::for_loopback_test(NetworkKey::generate().unwrap()))
            .await
            .unwrap();
        // when: the configuration is asked for twice
        let (first, second) = (node.proxy_config(), node.proxy_config());
        let id = node.stack.me();
        first.pool.note_gateway_refused(id);
        let (same_slots, same_pool) = (
            Arc::ptr_eq(&first.udp_slots, &second.udp_slots),
            second.pool.gateway_refused_recently(id),
        );
        node.shutdown().await;
        // then: both calls give the same limit and the same pool
        assert!(same_slots, "proxy_config() must return clones of one configuration");
        assert!(same_pool, "the pool of both configurations must be shared");
    }
}

/// What `serve_full` leaves of the serving phase after the node is torn down.
struct Served {
    addr: Option<SocketAddr>,
    lines: Vec<String>,
    proxy: crate::proxy::ProxyConfig,
    id: crate::transport::net::PeerId,
}

/// Like `serve_with`, plus the proxy configuration and the node's id.
async fn serve_full(socks_enabled: bool) -> Served {
    let _log = LOG_LOCK.lock().await;
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    cfg.socks.enabled = socks_enabled;
    let mut boot = boot_endpoint(&cfg).await.unwrap();
    let (net, inbound) = spawn_network(&cfg, &boot).await.unwrap();
    let access = super::super::access_wiring::access_start(&cfg, boot.id, &inbound.cache);
    let after = log_page(0, 1).last_seq;
    let serving = setup_serving(&cfg, &mut boot, &net, access).await.unwrap();
    let lines = log_page(after, 10_000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "socks")
        .map(|l| l.text)
        .collect();
    boot.background_abort_handles.iter().for_each(|t| t.abort());
    serving.socks_task.close().await;
    net.stack.close().await;
    Served {
        addr: serving.socks_addr,
        lines,
        proxy: serving.proxy,
        id: boot.id,
    }
}
