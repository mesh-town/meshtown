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

fn run_after(cmd: Cmd) -> (ControlFlow<()>, NodeConfig) {
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

/// Network switch (`boot_cycle` in a row) on one fixed SOCKS5 port.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn boot_cycles_reuse_fixed_socks_port() {
    let ctl = NodeCtl::new(&Settings::default());
    let port = crate::node::socks_task::tests::free_port();
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([4; 32]));
    cfg.socks.enabled = true;
    cfg.socks.port = port;
    for round in 0..20 {
        let node = boot_cycle(&cfg, &ctl, false)
            .await
            .unwrap_or_else(|e| panic!("round {round}, port {port}: {e:#}"));
        ctl.stop_on_os_signal();
        assert!(
            matches!(serve_until_command(node, &ctl).await, Cmd::Stop),
            "round {round}"
        );
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
