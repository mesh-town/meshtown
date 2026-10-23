use super::*;
use crate::members::network_key::NetworkKey;
use crate::node::identity::TemporaryReason;
use crate::transport::net::NodeSecret;
use crate::web_gui::NodeControl;
use std::time::Duration;
use tokio::time::timeout;

fn test_relaunch() -> crate::node::node_ctl::Relaunch {
    crate::node::node_ctl::Relaunch {
        file: None,
        cli: crate::config::config_file::CliOverrides::default(),
        secret: NodeSecret::from_bytes(&[11u8; 32]),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    }
}

/// (a) `stop()` before calling `run_node_awaiting_key`, (b) `stop()` 50 ms after `spawn` — both times
/// `Ok(())` within a `timeout` of 2 s.
#[tokio::test]
async fn run_after_key_stop_before_and_during_wait() {
    use crate::web_gui::NodeControl;

    let settings = crate::config::config_file::Settings::default();
    let ctl = crate::node::node_ctl::NodeCtl::new_without_network(&settings);
    ctl.stop();
    let res = timeout(
        Duration::from_secs(2),
        run_node_awaiting_key(
            settings.clone(),
            IdentityState::Temporary(TemporaryReason::Declined),
            ctl,
            test_relaunch(),
        ),
    )
    .await;
    assert!(
        matches!(res, Ok(Ok(()))),
        "(a) stop() before the call: {res:?}"
    );

    let ctl = crate::node::node_ctl::NodeCtl::new_without_network(&settings);
    let ctl2 = ctl.clone();
    let task = crate::node::output::spawn(run_node_awaiting_key(
        settings,
        IdentityState::Temporary(TemporaryReason::Declined),
        ctl,
        test_relaunch(),
    ));
    tokio::time::sleep(Duration::from_millis(50)).await;
    ctl2.stop();
    let res = timeout(Duration::from_secs(2), task).await;
    assert!(
        matches!(res, Ok(Ok(Ok(())))),
        "(b) stop() after 50 ms: {res:?}"
    );
}

/// A bad key keeps the node waiting (`Offline`) without finishing the task; `stop()` still
/// ends it with `Ok(())`.
#[tokio::test]
async fn run_after_key_bad_key_keeps_waiting() {
    use crate::web_gui::NodeControl;

    let settings = crate::config::config_file::Settings::default();
    let ctl = crate::node::node_ctl::NodeCtl::new_without_network(&settings);
    ctl.join_with_peers("not a key at all", vec![]);
    let ctl2 = ctl.clone();
    let task = crate::node::output::spawn(run_node_awaiting_key(
        settings,
        IdentityState::Temporary(TemporaryReason::Declined),
        ctl,
        test_relaunch(),
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        ctl2.snapshot().node.status,
        crate::web_gui::NodeStatus::Offline
    );
    assert!(!task.is_finished(), "the task must not finish on a bad key");
    ctl2.stop();
    let res = timeout(Duration::from_secs(2), task).await;
    assert!(
        matches!(res, Ok(Ok(Ok(())))),
        "stop() after a bad key: {res:?}"
    );
}

fn peer(b: u8) -> PeerAddr {
    PeerAddr::new(NodeSecret::from_bytes(&[b; 32]).peer_id())
}

fn key_text(b: u8) -> String {
    NetworkKey::from_bytes([b; 32]).encode()
}

fn old_cfg() -> NodeConfig {
    let mut cfg =
        test_relaunch().node_config(&Settings::default(), NetworkKey::from_bytes([1; 32]));
    cfg.identity = IdentityState::Persistent;
    cfg.initial_peers = vec![peer(1)];
    cfg
}

fn old_settings() -> Settings {
    Settings {
        net: crate::config::config_file::NetSettings {
            network_key: Some(key_text(1)),
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    }
}

#[tokio::test]
async fn apply_switch_bad_key_changes_nothing() {
    let mut settings = old_settings();
    let ctl = NodeCtl::new_without_network(&settings);
    let res = apply_switch(
        "not a key at all",
        vec![peer(2)],
        IdentityState::Persistent,
        &mut settings,
        &ctl,
        &test_relaunch(),
    );
    assert!(res.is_err());
    assert_eq!(settings.net.network_key, Some(key_text(1)));
}

#[tokio::test]
async fn apply_switch_good_key_takes_identity_and_peers() {
    let mut settings = old_settings();
    let ctl = NodeCtl::new_without_network(&settings);
    let cfg = apply_switch(
        &key_text(2),
        vec![peer(2)],
        IdentityState::Persistent,
        &mut settings,
        &ctl,
        &test_relaunch(),
    )
    .unwrap();
    assert_eq!(cfg.identity, IdentityState::Persistent);
    assert_eq!(cfg.initial_peers, vec![peer(2)]);
    assert_eq!(cfg.network_key.encode(), key_text(2));
    assert_eq!(settings.net.network_key, Some(key_text(2)));
}

fn run_after(cmd: Cmd) -> (ControlFlow<CycleEnd>, NodeConfig) {
    let mut settings = old_settings();
    let ctl = NodeCtl::new_without_network(&settings);
    let mut cfg = old_cfg();
    let flow = after_command(cmd, &mut cfg, &mut settings, &ctl, &test_relaunch());
    (flow, cfg)
}

fn assert_old(cfg: &NodeConfig) {
    assert_eq!(cfg.network_key.encode(), key_text(1));
    assert_eq!(cfg.identity, IdentityState::Persistent);
    assert_eq!(cfg.initial_peers, vec![peer(1)]);
}

#[tokio::test]
async fn after_command_stop_breaks() {
    let (flow, cfg) = run_after(Cmd::Stop);
    assert!(flow.is_break());
    assert_old(&cfg);
}

#[tokio::test]
async fn after_command_bad_key_keeps_old_cfg() {
    let (flow, cfg) = run_after(Cmd::Switch {
        key: "not a key at all".into(),
        peers: vec![peer(2)],
    });
    assert!(flow.is_continue());
    assert_old(&cfg);
}

#[tokio::test]
async fn after_command_good_key_replaces_cfg_keeps_identity() {
    let (flow, cfg) = run_after(Cmd::Switch {
        key: key_text(2),
        peers: vec![peer(2)],
    });
    assert!(flow.is_continue());
    assert_eq!(cfg.network_key.encode(), key_text(2));
    assert_eq!(cfg.identity, IdentityState::Persistent);
    assert_eq!(cfg.initial_peers, vec![peer(2)]);
}

/// Every cycle goes through `boot_cycle` and takes a new start id (no external network, loopback).
#[tokio::test]
async fn each_boot_cycle_takes_new_boot_id() {
    let ctl = NodeCtl::new(&Settings::default());
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([3; 32]));
    cfg.socks.enabled = false;
    let id0 = ctl.snapshot().boot_id;
    let mut prev = id0;
    for _ in 0..2 {
        let node = boot_cycle(&cfg, &ctl, false).await.unwrap();
        let id = ctl.snapshot().boot_id;
        assert_ne!(id, prev);
        prev = id;
        ctl.stop_on_os_signal();
        assert!(matches!(serve_until_command(node, &ctl).await, Cmd::Stop));
    }
}

/// A stop that arrives while the node starts survives `attach`: the snapshot says "stopping".
#[tokio::test]
async fn stop_before_attach_keeps_stopping_status() {
    let ctl = NodeCtl::new(&Settings::default());
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([5; 32]));
    cfg.socks.enabled = false;
    ctl.stop_on_os_signal();
    let node = boot_cycle(&cfg, &ctl, true).await.unwrap();
    assert_eq!(
        ctl.snapshot().node.status,
        crate::web_gui::NodeStatus::Stopping
    );
    assert!(matches!(serve_until_command(node, &ctl).await, Cmd::Stop));
}

/// With the stop queued before the start, the loop leaves after that one start and takes no second boot id.
#[tokio::test]
async fn stop_before_attach_leaves_after_one_boot() {
    let ctl = NodeCtl::new(&Settings::default());
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([6; 32]));
    cfg.socks.enabled = false;
    let mut settings = Settings::default();
    let id0 = ctl.snapshot().boot_id;
    ctl.stop_on_os_signal();
    let res = timeout(
        Duration::from_secs(20),
        run_cycles(&mut cfg, &mut settings, &ctl, &test_relaunch()),
    )
    .await
    .expect("loop must leave");
    assert!(res.is_ok());
    let id1 = ctl.snapshot().boot_id;
    assert_ne!(id0, id1);
}

/// Leaving the network of a running node: the loop ends with `LeftNet`, the settings lose the network's
/// key and lists, and the control object waits for a key again.
#[tokio::test]
async fn leave_net_ends_cycles_and_clears_network_settings() {
    let mut settings = Settings::default();
    settings.net.route_exits = vec![NodeSecret::from_bytes(&[9u8; 32]).peer_id()];
    let ctl = NodeCtl::new(&settings);
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([7; 32]));
    cfg.socks.enabled = false;
    let new_secret = NodeSecret::from_bytes(&[33u8; 32]);
    ctl.request_leave_net(new_secret.clone()).unwrap();
    let res = timeout(
        Duration::from_secs(20),
        run_cycles(&mut cfg, &mut settings, &ctl, &test_relaunch()),
    )
    .await
    .expect("loop must leave")
    .unwrap();
    let CycleEnd::LeftNet { secret } = res else {
        panic!("expected LeftNet");
    };
    assert_eq!(secret.peer_id(), new_secret.peer_id());
    let relaunch = forget_network(&mut settings, &ctl, &test_relaunch(), secret);
    assert_eq!(relaunch.secret.peer_id(), new_secret.peer_id());
    assert!(settings.net.network_key.is_none());
    assert!(settings.net.route_exits.is_empty());
    assert!(settings.net.ports_published.is_empty());
    assert!(ctl.is_no_network());
}

/// A running node that is told to leave waits for a key; a stop ends the wait.
#[tokio::test]
async fn left_node_waits_for_key_until_stop() {
    use crate::web_gui::NodeControl;

    let settings = Settings::default();
    let ctl = NodeCtl::new(&settings);
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([8; 32]));
    cfg.socks.enabled = false;
    let task = crate::node::output::spawn(run_node_with_ctl(
        cfg,
        settings,
        ctl.clone(),
        test_relaunch(),
    ));
    tokio::time::sleep(Duration::from_millis(300)).await;
    ctl.request_leave_net(NodeSecret::from_bytes(&[34u8; 32]))
        .unwrap();
    let waited = timeout(Duration::from_secs(20), async {
        while !ctl.is_no_network() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        waited.is_ok(),
        "the node must come to waiting for a network key"
    );
    assert!(!task.is_finished());
    ctl.stop();
    let res = timeout(Duration::from_secs(5), task).await;
    assert!(matches!(res, Ok(Ok(Ok(())))), "{res:?}");
}

/// The cycle loop and the process proxy: the node publishes its configuration to the listener it does not own,
/// and the owner slot brings the listener to the settings before every start.
mod process_proxy {
    use super::*;

    mod node_up {
        use super::*;

        /// given a listener without a target, when a cycle starts, then the listener's target is that node's path.
        #[tokio::test]
        async fn after_the_start_the_front_serves_the_node_path() {
            let fx = setup(51).await;
            fx.then_target_is(None);

            let node = fx.when_cycle_starts(fx.port, 1).await;

            fx.then_target_is(Some(1));
            fx.when_cycle_is_stopped(node).await;
        }
    }

    mod node_down {
        use super::*;

        /// given a node that serves the listener, when it is told to stop, then the target is gone while
        /// the shutdown is still running: new connections are refused, not sent into a closing node.
        #[tokio::test]
        async fn the_front_has_no_target_before_the_shutdown_ends() {
            let fx = setup(52).await;
            let node = fx.when_cycle_starts(fx.port, 1).await;
            fx.ctl.stop_on_os_signal();
            let mut ending = Box::pin(serve_until_command(node, &fx.ctl));

            let first_poll = timeout(Duration::ZERO, &mut ending).await;

            assert!(first_poll.is_err(), "the shutdown must still be running");
            fx.then_target_is(None);
            assert!(matches!(ending.await, Cmd::Stop));
        }

        #[tokio::test]
        async fn the_front_stays_without_a_target_after_the_stop() {
            let fx = setup(53).await;
            let node = fx.when_cycle_starts(fx.port, 1).await;

            fx.when_cycle_is_stopped(node).await;

            fx.then_target_is(None);
        }
    }

    mod restart_with_a_new_socks_port {
        use super::*;

        /// given a node up on port A, when the restart brings settings with port B, then the front listens on B,
        /// A is free again, and the new node's path is the target.
        #[tokio::test]
        async fn the_front_moves_to_the_new_port_and_serves_the_new_node() {
            let fx = setup(54).await;
            let (old_port, new_port) = (fx.port, free_port());
            let node = fx.when_cycle_starts(old_port, 1).await;
            let cmd = fx.when_cycle_is_restarted(node).await;
            assert!(matches!(cmd, Cmd::Switch { .. }), "restart: {cmd:?}");

            let node = fx.when_cycle_starts(new_port, 2).await;

            assert_eq!(fx.front_port(), Some(new_port));
            assert!(can_bind(old_port), "the old port must be released");
            assert!(!can_bind(new_port), "the front must hold the new port");
            fx.then_target_is(Some(2));
            fx.when_cycle_is_stopped(node).await;
        }

        #[tokio::test]
        async fn the_same_port_keeps_the_front_where_it_was() {
            let fx = setup(55).await;
            let node = fx.when_cycle_starts(fx.port, 1).await;
            fx.when_cycle_is_restarted(node).await;

            let node = fx.when_cycle_starts(fx.port, 2).await;

            assert_eq!(fx.front_port(), Some(fx.port));
            fx.then_target_is(Some(2));
            fx.when_cycle_is_stopped(node).await;
        }
    }

    mod new_port_is_taken_by_another_program {
        use super::*;

        /// given a node up, when the next start brings a busy port, then the start fails with the bind error,
        /// the old port still listens and the front has no target (the node it served is not the one being started).
        #[tokio::test]
        async fn the_start_fails_and_the_front_keeps_its_port_without_a_target() {
            let fx = setup(56).await;
            let node = fx.when_cycle_starts(fx.port, 1).await;
            let foreign = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let busy = foreign.local_addr().unwrap().port();

            let err = fx.when_cycle_start_fails(busy, 2).await;

            let bind_error = err
                .downcast_ref::<crate::proxy::BindError>()
                .expect("a BindError");
            assert_eq!(bind_error.port, busy);
            assert_eq!(
                fx.front_port(),
                Some(fx.port),
                "the old listener keeps working"
            );
            fx.then_target_is(None);
            node.shutdown().await;
        }
    }
}

/// A process listener on a free port, a seat for the `main` slot over it and a controller that publishes there.
struct Cycle {
    ctl: Arc<NodeCtl>,
    front: crate::proxy::ProxyFront,
    port: u16,
    key_byte: u8,
}

/// `key_byte` makes the network of the test's nodes its own: tests run side by side in one in-memory network.
async fn setup(key_byte: u8) -> Cycle {
    let port = free_port();
    let front = crate::proxy::ProxyFront::bind(&process_on(port))
        .await
        .expect("a free loopback port");
    let seat = crate::proxy::ProxySeat::new(
        Some(front.clone()),
        crate::config::config_file::NetSlotId::main(),
    );
    let settings = Settings {
        net: crate::config::config_file::NetSettings {
            network_key: Some(key_text(key_byte)),
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    let ctl = NodeCtl::new(&settings);
    ctl.set_proxy_seat(
        crate::config::config_file::NetSlotId::main(),
        Arc::new(seat),
    );
    Cycle {
        ctl,
        front,
        port,
        key_byte,
    }
}

impl Cycle {
    /// The node of this cycle; its chosen exit list is `exit_mark(mark)`, which tells nodes apart.
    fn node_config(&self, mark: u8) -> NodeConfig {
        let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([self.key_byte; 32]));
        cfg.settings.route_exits = vec![exit_mark(mark)];
        cfg.proxy_face = self.ctl.proxy_face();
        cfg
    }

    async fn when_cycle_starts(&self, socks_port: u16, mark: u8) -> RunningNode {
        self.start_cycle(socks_port, mark)
            .await
            .expect("the cycle starts")
    }

    async fn when_cycle_start_fails(&self, socks_port: u16, mark: u8) -> anyhow::Error {
        let started = self.start_cycle(socks_port, mark).await;
        started.err().expect("the cycle must not start")
    }

    /// The start of a cycle the way `run_cycles` does it: the process proxy is brought to the settings
    /// the cycle comes up with, then the node boots.
    async fn start_cycle(&self, socks_port: u16, mark: u8) -> Result<RunningNode> {
        apply_proxy(&settings_on(socks_port), &self.ctl, false).await?;
        boot_cycle(&self.node_config(mark), &self.ctl, false).await
    }

    async fn when_cycle_is_stopped(&self, node: RunningNode) {
        self.ctl.stop_on_os_signal();
        assert!(matches!(
            serve_until_command(node, &self.ctl).await,
            Cmd::Stop
        ));
    }

    /// The page's "restart" button: the node stops with a `Switch` to the same key.
    async fn when_cycle_is_restarted(&self, node: RunningNode) -> Cmd {
        self.ctl.restart().expect("the node is up");
        serve_until_command(node, &self.ctl).await
    }

    fn front_port(&self) -> Option<u16> {
        self.front.addr().map(|addr| addr.port())
    }

    /// The target of the listener is told by the node's exit list; `None` — no target.
    fn then_target_is(&self, mark: Option<u8>) {
        let want = mark.map(|mark| vec![exit_mark(mark)]);
        let got = self.front.current_route().map(|route| route.chosen_exits());
        assert_eq!(got, want, "the listener's target (marked by its exit list)");
    }
}

fn exit_mark(mark: u8) -> crate::transport::net::PeerId {
    NodeSecret::from_bytes(&[mark; 32]).peer_id()
}

fn process_on(port: u16) -> crate::config::config_file::ProcessSettings {
    crate::config::config_file::ProcessSettings {
        socks_port: port,
        ..crate::config::config_file::ProcessSettings::default()
    }
}

fn settings_on(port: u16) -> Settings {
    Settings {
        process: process_on(port),
        ..Settings::default()
    }
}

/// A free port on loopback: bind with port zero, remember it, close.
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn can_bind(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}
