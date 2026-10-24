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

#[path = "lifecycle_network_tests.rs"]
mod network;

#[path = "lifecycle_switch_tests.rs"]
mod switch;

#[path = "lifecycle_proxy_support_tests.rs"]
mod proxy_support;

#[path = "lifecycle_proxy_tests.rs"]
mod proxy;

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

fn key_text(b: u8) -> String {
    NetworkKey::from_bytes([b; 32]).encode()
}
/// Every cycle goes through `boot_cycle` and takes a new start id (no external network, loopback).
#[tokio::test]
async fn each_boot_cycle_takes_new_boot_id() {
    let ctl = NodeCtl::new(&Settings::default());
    let cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([3; 32]));
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
    let cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([5; 32]));
    ctl.stop_on_os_signal();
    let node = boot_cycle(&cfg, &ctl, true).await.unwrap();
    assert_eq!(
        ctl.snapshot().node.status,
        crate::web_gui::NodeStatus::Stopping
    );
    assert!(matches!(serve_until_command(node, &ctl).await, Cmd::Stop));
}

/// With the stop queued before the loop starts, the loop leaves at once and the node takes no boot id.
#[tokio::test]
async fn stop_before_the_loop_takes_no_boot() {
    let ctl = NodeCtl::new(&Settings::default());
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([6; 32]));
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
    assert_eq!(id0, id1, "the node must not boot");
}

/// Leaving the network of a running node: the loop ends with `LeftNet`, the settings lose the network's
/// key and lists, and the control object waits for a key again.
#[tokio::test]
async fn leave_net_ends_cycles_and_clears_network_settings() {
    let mut settings = Settings::default();
    settings.net.route_exits = vec![NodeSecret::from_bytes(&[9u8; 32]).peer_id()];
    let ctl = NodeCtl::new(&settings);
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([7; 32]));
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
    let cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([8; 32]));
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
