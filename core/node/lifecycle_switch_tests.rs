use super::*;
use crate::transport::net::NodeSecret;

fn peer(b: u8) -> PeerAddr {
    PeerAddr::new(NodeSecret::from_bytes(&[b; 32]).peer_id())
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
