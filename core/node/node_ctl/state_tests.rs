use super::*;
use crate::members::network_key::NetworkKey;
use crate::node::NodeConfig;
use crate::transport::net::endpoint_shim::{EndpointAddr, TransportAddr};
use crate::transport::net::NodeSecret;
use crate::web_gui::NodeControl;
use std::net::SocketAddr;
use std::time::Duration;

/// A test member address: the same approach as in `peer_lines_and_parse_peer_line_roundtrip`.
fn test_addr(byte: u8) -> crate::transport::net::PeerAddr {
    let id = NodeSecret::from_bytes(&[byte; 32]).peer_id();
    let addr: SocketAddr = format!("203.0.113.{byte}:4433").parse().unwrap();
    crate::transport::net::endpoint_shim::from_endpoint_addr(&EndpointAddr::from_parts(
        id,
        [TransportAddr::Ip(addr)],
    ))
}

#[tokio::test]
async fn actions_before_attach_are_busy() {
    let ctl = NodeCtl::new(&Settings::default());
    let busy_starting = ActionError::Busy("Mesh Town is starting".to_string());
    assert_eq!(ctl.find_exit("example.com"), Err(busy_starting.clone()));
    assert_eq!(ctl.check_domain("example.com"), Err(busy_starting.clone()));
    assert_eq!(ctl.check_all(), Err(busy_starting.clone()));
    assert_eq!(ctl.add_domain("example.com"), Err(busy_starting.clone()));
    assert_eq!(ctl.remove_domain("example.com"), Err(busy_starting));
    assert_eq!(
        ctl.add_peer_address("x@1.2.3.4:1"),
        Err("Mesh Town has not started yet".to_string())
    );
    let snap = ctl.snapshot();
    assert_eq!(snap.node.status, NodeStatus::Starting);
    assert_eq!(snap.boot_id.len(), 8);
    assert!(snap.boot_id.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn next_command_folds_switch_and_stop() {
    let ctl = NodeCtl::new(&Settings::default());
    ctl.switch_network("a");
    ctl.switch_network("b");
    assert_eq!(
        ctl.next_command().await,
        Cmd::Switch {
            key: "b".to_string(),
            peers: vec![]
        }
    );

    ctl.switch_network("c");
    ctl.stop();
    assert_eq!(ctl.next_command().await, Cmd::Stop);

    ctl.switch_network("d");
    let res = tokio::time::timeout(Duration::from_millis(50), ctl.next_command()).await;
    assert!(
        res.is_err(),
        "after stop() switch_network must not send a command"
    );
}

/// The invite addresses belong wholly to the last `Switch`: `peers` of different
/// `next_command` calls must not be mixed, even if `join_with_peers` without addresses happened between them.
#[tokio::test]
async fn next_command_keeps_peers_of_last_switch() {
    let ctl = NodeCtl::new(&Settings::default());
    let p1 = test_addr(1);
    let p2 = test_addr(2);

    ctl.join_with_peers("a", vec![p1.clone()]);
    ctl.join_with_peers("b", vec![p2.clone()]);
    assert_eq!(
        ctl.next_command().await,
        Cmd::Switch {
            key: "b".to_string(),
            peers: vec![p2.clone()]
        }
    );

    ctl.join_with_peers("c", vec![p1.clone()]);
    ctl.switch_network("d");
    assert_eq!(
        ctl.next_command().await,
        Cmd::Switch {
            key: "d".to_string(),
            peers: vec![]
        }
    );
}

#[tokio::test]
async fn no_network_is_offline_and_busy() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    assert_eq!(ctl.snapshot().node.status, NodeStatus::Offline);
    assert_eq!(
        ctl.invite_pending(None).err(),
        Some(ActionError::Busy(format!(
            "this computer has not joined the {0} network yet",
            crate::node::output::NET_NAME
        )))
    );
    assert_eq!(
        ctl.find_exit("example.com"),
        Err(ActionError::Busy(format!(
            "this computer has not joined the {0} network yet: paste an invite from a network member or create your own",
            crate::node::output::NET_NAME
        )))
    );
}

#[tokio::test]
async fn join_from_no_network_goes_starting() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    let p1 = test_addr(1);
    ctl.join_with_peers("mesh1-abc", vec![p1.clone()]);
    assert_eq!(ctl.snapshot().node.status, NodeStatus::Starting);
    assert_eq!(
        ctl.next_command().await,
        Cmd::Switch {
            key: "mesh1-abc".to_string(),
            peers: vec![p1]
        }
    );
}

#[tokio::test]
async fn stop_from_no_network_queues_stop() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    ctl.stop();
    assert_eq!(ctl.snapshot().node.status, NodeStatus::Stopping);
    assert_eq!(ctl.next_command().await, Cmd::Stop);
}

#[tokio::test]
async fn mark_no_network_keeps_stopping() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    ctl.stop();
    ctl.mark_no_network();
    assert_eq!(ctl.snapshot().node.status, NodeStatus::Stopping);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_config_before_attach_becomes_pending() {
    let ctl = NodeCtl::new(&Settings::default());
    let settings = Settings {
        net: crate::config::config_file::NetSettings {
            exit_enabled: true,
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    ctl.apply_config(&settings, &["exit.enabled"]);

    let key = NetworkKey::generate().unwrap();
    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.settings.exit_enabled = false;
    let node = crate::node::start_node(cfg).await.unwrap();
    assert!(!node.exit_on.load(std::sync::atomic::Ordering::Relaxed));

    ctl.attach(&node);
    assert!(
        node.exit_on.load(std::sync::atomic::Ordering::Relaxed),
        "a deferred exit.enabled must apply set_exit_enabled(true) in attach"
    );

    ctl.detach(NodeStatus::Stopping);
    node.shutdown().await;
}

/// The node name from `node.name` reaches the live node through the `set_node_name` handle:
/// both when deferred until `attach` and when it arrives after it.
mod node_name_path {
    use super::*;

    const NEW_NAME: &str = "new-name";

    // fails if the `node.name` branch is missing from `apply_pending_handles` (the name stays the initial one)
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn name_before_attach_is_applied_on_attach() {
        // given: the name is set before the node starts, the node has the initial name — the default
        let ctl = NodeCtl::new(&Settings::default());
        ctl.apply_config(&settings_with_name(NEW_NAME), &["node.name"]);
        let node = start_test_node().await;
        assert_ne!(
            node.current_name(),
            NEW_NAME,
            "precondition: the name is not applied yet"
        );

        // when: the node is attached to `NodeCtl`
        ctl.attach(&node);

        // then: the deferred name is applied by the handle
        assert_eq!(
            node.current_name(),
            NEW_NAME,
            "a deferred node.name must apply set_node_name in attach"
        );
        ctl.detach(NodeStatus::Stopping);
        node.shutdown().await;
    }

    // fails if `apply_config` does not carry `node.name` to the handle on an attached node
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn name_after_attach_is_applied_at_once() {
        // given: the node is already attached, it has the initial name — the default
        let ctl = NodeCtl::new(&Settings::default());
        let node = start_test_node().await;
        ctl.attach(&node);
        assert_ne!(
            node.current_name(),
            NEW_NAME,
            "precondition: the name is not applied yet"
        );

        // when: the page saves a new name
        ctl.apply_config(&settings_with_name(NEW_NAME), &["node.name"]);

        // then: the node is renamed right away
        assert_eq!(
            node.current_name(),
            NEW_NAME,
            "node.name after attach must be applied at once"
        );
        ctl.detach(NodeStatus::Stopping);
        node.shutdown().await;
    }

    // --- setup and helpers ---

    fn settings_with_name(name: &str) -> Settings {
        Settings {
            net: crate::config::config_file::NetSettings {
                node_name: Some(name.to_string()),
                ..crate::config::config_file::NetSettings::default()
            },
            ..Settings::default()
        }
    }

    /// A loopback node without its own name in the config (the default name), without SOCKS.
    async fn start_test_node() -> RunningNode {
        let key = NetworkKey::generate().unwrap();
        let cfg = NodeConfig::for_loopback_test(key);
        crate::node::start_node(cfg).await.unwrap()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invite_from_attached_node_parses_back() {
    let ctl = NodeCtl::new(&Settings::default());
    let key = NetworkKey::generate().unwrap();
    let node = crate::node::start_node(NodeConfig::for_loopback_test(key.clone()))
        .await
        .unwrap();
    ctl.attach(&node);

    let invite = ctl
        .invite_pending(None)
        .expect("invite_pending() of a node on the network")
        .finish()
        .await
        .expect("the issue record in the network document succeeded");
    let parsed = crate::invite::parse(&invite).expect("the invite parses");
    assert_eq!(parsed.head.network_id, key.subkeys().network_id);
    assert!(parsed.peers.iter().any(|p| p.id == node.id));

    ctl.detach(NodeStatus::Stopping);
    node.shutdown().await;
}

/// An invite from a node on the network: a string without the network key, and the issue is written to the document (`finish`).
#[path = "state_invite_tests.rs"]
mod invite_pending_of_attached_node;

/// The "Restart node" button: `request_restart` sends `Switch` with the current key.
#[path = "state_restart_tests.rs"]
mod request_restart;

/// The cycle stamp in the slot: `boot_id` and `uptime_sec` live in `NodeCtlState`, changed only by `renew_boot_stamp`.
#[path = "state_boot_tests.rs"]
mod boot_stamp;

/// The process proxy shows in the snapshot of a controller that has no node, so the page can still tell
/// where the listener is.
mod socks_without_a_node {
    use super::*;
    use crate::config::config_file::{NetSlotId, ProcessSettings};
    use crate::proxy::{ProxyFront, ProxySeat};

    mod seat_with_a_bound_front {
        use super::*;

        #[tokio::test]
        async fn socks_is_enabled_and_listens_on_the_front_address() {
            // given
            let (ctl, front) = setup(Seat::WithFront).await;

            // when
            let socks = ctl.snapshot().socks;

            // then
            let addr = front
                .expect("the front is bound")
                .addr()
                .expect("a bound address");
            assert!(socks.enabled, "a bound front means SOCKS is on");
            assert_eq!(socks.listen, Some(addr.to_string()));
        }
    }

    mod seat_without_a_front {
        use super::*;

        #[tokio::test]
        async fn socks_is_disabled_and_has_no_address() {
            // given
            let (ctl, _) = setup(Seat::WithoutFront).await;

            // when
            let socks = ctl.snapshot().socks;

            // then
            assert!(!socks.enabled);
            assert_eq!(socks.listen, None);
        }
    }

    mod no_seat {
        use super::*;

        #[tokio::test]
        async fn socks_is_disabled_and_has_no_address() {
            // given
            let (ctl, _) = setup(Seat::None).await;

            // when
            let socks = ctl.snapshot().socks;

            // then
            assert!(!socks.enabled);
            assert_eq!(socks.listen, None);
        }
    }

    // --- setup and helpers ---

    enum Seat {
        WithFront,
        WithoutFront,
        None,
    }

    /// A controller without a network and a node; the seat (if any) sits in it. Also gives the front back.
    async fn setup(seat: Seat) -> (Arc<NodeCtl>, Option<ProxyFront>) {
        let ctl = NodeCtl::new_without_network(&Settings::default());
        let front = match seat {
            Seat::WithFront => Some(bind_front().await),
            _ => None,
        };
        if !matches!(seat, Seat::None) {
            let seat = Arc::new(ProxySeat::new(front.clone(), NetSlotId::main()));
            ctl.set_proxy_seat(NetSlotId::main(), seat);
        }
        (ctl, front)
    }

    async fn bind_front() -> ProxyFront {
        ProxyFront::bind(&ProcessSettings {
            socks_port: 0,
            ..ProcessSettings::default()
        })
        .await
        .expect("a free loopback port")
    }
}
