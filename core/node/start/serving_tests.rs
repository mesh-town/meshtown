use super::*;
use crate::members::network_key::NetworkKey;
use crate::node::start::boot::boot_endpoint;
use crate::node::start::network::spawn_network;

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

/// The proxy configuration the serving phase builds for a front: shared by every clone, with or without the
/// node's own listener.
mod proxy_config {
    use super::*;
    use crate::node::start_node;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_built_without_any_listener() {
        // given: the serving phase, which binds no listener
        let served = serve_full().await;
        // then: the configuration is there and its association limit is untouched
        assert_eq!(
            served.proxy.udp_slots.available_permits(),
            crate::proxy::udp_slots().available_permits(),
            "a fresh configuration must have all UDP association slots free"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn clone_shares_udp_slots() {
        // given: a serving phase
        let served = serve_full().await;
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
        let served = serve_full().await;
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
        let (first, second) = (serve_full().await, serve_full().await);
        // then: each has its own association limit
        assert!(
            !Arc::ptr_eq(&first.proxy.udp_slots, &second.proxy.udp_slots),
            "nodes must not share one UDP association limit"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn running_node_hands_out_configs_with_the_same_udp_slots() {
        // given: a started node
        let node = start_node(NodeConfig::for_loopback_test(
            NetworkKey::generate().unwrap(),
        ))
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
        assert!(
            same_slots,
            "proxy_config() must return clones of one configuration"
        );
        assert!(same_pool, "the pool of both configurations must be shared");
    }
}

/// What `serve_full` leaves of the serving phase after the node is torn down.
struct Served {
    proxy: crate::proxy::ProxyConfig,
    id: crate::transport::net::PeerId,
}

/// Runs the serving phase and tears the node down; keeps the proxy configuration and the node's id.
async fn serve_full() -> Served {
    let cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    let mut boot = boot_endpoint(&cfg).await.unwrap();
    let (net, inbound) = spawn_network(&cfg, &boot).await.unwrap();
    let access = super::super::access_wiring::access_start(&cfg, boot.id, &inbound.cache);
    let serving = setup_serving(&cfg, &mut boot, &net, access).await.unwrap();
    boot.background_abort_handles.iter().for_each(|t| t.abort());
    net.stack.close().await;
    Served {
        proxy: serving.proxy,
        id: boot.id,
    }
}
