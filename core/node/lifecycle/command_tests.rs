use super::*;
use crate::members::network_key::NetworkKey;
use crate::transport::net::NodeSecret;

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
    let key = NetworkKey::from_bytes([2; 32]).encode();
    let cmd = Cmd::Switch {
        key,
        peers: Vec::new(),
    };
    let flow = after_command(cmd, &mut cfg, &mut settings, &ctl, &relaunch());
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
        cfg.network_key.encode(),
        NetworkKey::from_bytes([2; 32]).encode()
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
    let flow = after_command(cmd, &mut cfg, &mut settings, &ctl, &relaunch());
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
    let flow = after_command(Cmd::Stop, &mut cfg, &mut settings, &ctl, &relaunch());
    assert!(matches!(flow, ControlFlow::Break(CycleEnd::Stopped)));
}
