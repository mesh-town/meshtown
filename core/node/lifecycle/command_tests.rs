use super::*;
use crate::members::network_key::NetworkKey;
use crate::transport::net::NodeSecret;
use crate::web_gui::NodeControl;

fn relaunch() -> Relaunch {
    Relaunch {
        file: None,
        cli: crate::config::config_file::CliOverrides::default(),
        secret: NodeSecret::from_bytes(&[11u8; 32]),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    }
}

fn run_switch(stop: bool) -> (ControlFlow<CycleEnd>, NodeConfig) {
    let mut settings = Settings::default();
    let ctl = NodeCtl::new(&settings);
    ctl.detach(crate::web_gui::NodeStatus::Starting);
    if stop {
        ctl.stop_on_os_signal();
    }
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([1; 32]));
    let key = crate::node::host::test_keys::managed_text(2);
    let cmd = Cmd::Switch {
        key,
        peers: Vec::new(),
        secret: None,
    };
    let flow = after_command(cmd, &mut cfg, &mut settings, &ctl, &mut relaunch());
    (flow, cfg)
}

#[tokio::test]
async fn restart_with_stop_in_start_window_breaks() {
    let (flow, _) = run_switch(true);
    assert!(flow.is_break());
}

#[tokio::test]
async fn restart_without_stop_continues_with_new_cfg() {
    let (flow, cfg) = run_switch(false);
    assert!(flow.is_continue());
    assert_eq!(
        cfg.network_key.to_bytes(),
        crate::node::host::test_keys::key_input(2).key.to_bytes()
    );
}

#[tokio::test]
async fn leave_net_breaks_with_the_same_secret() {
    let mut settings = Settings::default();
    let ctl = NodeCtl::new(&settings);
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([1; 32]));
    let secret = NodeSecret::from_bytes(&[42u8; 32]);
    let cmd = Cmd::LeaveNet {
        secret: secret.clone(),
    };
    let flow = after_command(cmd, &mut cfg, &mut settings, &ctl, &mut relaunch());
    match flow {
        ControlFlow::Break(CycleEnd::LeftNet { secret: got }) => {
            assert_eq!(got.peer_id(), secret.peer_id())
        }
        _ => panic!("expected LeftNet"),
    }
}

#[tokio::test]
async fn stop_breaks_as_stopped() {
    let mut settings = Settings::default();
    let ctl = NodeCtl::new(&settings);
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([1; 32]));
    let flow = after_command(Cmd::Stop, &mut cfg, &mut settings, &ctl, &mut relaunch());
    assert!(matches!(flow, ControlFlow::Break(CycleEnd::Stopped)));
}

fn switch_with(secret: Option<NodeSecret>) -> Cmd {
    Cmd::Switch {
        key: crate::node::host::test_keys::managed_text(2),
        peers: Vec::new(),
        secret,
    }
}

#[tokio::test]
async fn after_switch_with_a_secret_replaces_only_this_relaunch() {
    // given: a node that was up (its id is remembered) and two start copies
    let mut settings = Settings::default();
    let ctl = NodeCtl::new(&settings);
    let node = crate::node::start_node(NodeConfig::for_loopback_test(
        NetworkKey::generate().unwrap(),
    ))
    .await
    .unwrap();
    ctl.attach(&node);
    ctl.detach(crate::web_gui::NodeStatus::Starting);
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([1; 32]));
    let (mut mine, other) = (relaunch(), relaunch());
    let new = NodeSecret::from_bytes(&[42u8; 32]);
    // when
    let _ = after_command(
        switch_with(Some(new.clone())),
        &mut cfg,
        &mut settings,
        &ctl,
        &mut mine,
    );
    // then
    assert_eq!(
        mine.secret.peer_id(),
        new.peer_id(),
        "this slot starts with the new key"
    );
    assert_eq!(
        other.secret.peer_id(),
        relaunch().secret.peer_id(),
        "other slots keep theirs"
    );
    let owner = ctl.owner_secret().expect("the owner is remembered");
    assert_eq!(
        owner.peer_id(),
        new.peer_id(),
        "the control state holds the new key"
    );
    assert_eq!(
        ctl.snapshot().node.id_full,
        "",
        "the remembered id belongs to the old key"
    );
    node.shutdown().await;
}

#[tokio::test]
async fn after_switch_without_a_secret_keeps_the_key() {
    let mut settings = Settings::default();
    let ctl = NodeCtl::new(&settings);
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::from_bytes([1; 32]));
    let mut mine = relaunch();
    let _ = after_command(switch_with(None), &mut cfg, &mut settings, &ctl, &mut mine);
    assert_eq!(mine.secret.peer_id(), relaunch().secret.peer_id());
}
