//! The start settings of a saved slot carry the peers its invite named.

use super::*;
use crate::config::config_file::{
    self, net_slot_path, new_slot_id, save_invited_peers, save_net_file, CliOverrides,
    ConfigSource, NetSettings, PathRule, TransportMode,
};
use crate::members::network_key::NetworkKey;
use crate::transport::net::NodeSecret;

const NOW: u64 = 1_790_000_000;

fn peer(n: u8) -> PeerAddr {
    PeerAddr::new(NodeSecret::from_bytes(&[n; 32]).peer_id())
}

/// Builds the launch of a slot saved beside a fresh main file; `peers` are saved with the slot when not empty.
fn launch_of_slot(name: &str, peers: &[PeerAddr]) -> SlotLaunch {
    let dir = std::env::temp_dir().join(format!("mesh-slot-invited-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let overrides = CliOverrides {
        network_key: Some(NetworkKey::from_bytes([61; 32]).encode()),
        transport_mode: Some(TransportMode::NoRelay),
        ..CliOverrides::default()
    };
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let main = config_file::prepare(&source, &overrides, true, 0).expect("main");
    let main_path = dir.join(config_file::FILE_NAME);
    let id = new_slot_id(&main_path).expect("id");
    let path = net_slot_path(&main_path, &id).unwrap();
    let net = NetSettings {
        network_key: Some(NetworkKey::from_bytes([62; 32]).encode()),
        transport_mode: TransportMode::NoRelay,
        ..NetSettings::default()
    };
    let secret = NodeSecret::from_bytes(&[62; 32]);
    save_net_file(&path, None, NOW, NOW, &secret, &net).expect("slot file");
    save_invited_peers(&path, peers).expect("peers");
    let slot = list_net_slots(&main_path).remove(0);
    let run = prepare_net_slot(&main, slot, NOW).expect("prepare");
    let relaunch = Relaunch {
        file: main.file.clone(),
        cli: overrides,
        secret: main.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: Some(crate::transport::net::mem::process_net()),
    };
    let parsed = slot_key(&run).expect("key");
    let launch = slot_launch(id, run, parsed, &relaunch);
    std::fs::remove_dir_all(&dir).ok();
    launch
}

#[test]
fn a_slot_with_invited_peers_starts_with_them() {
    let peers = vec![peer(1), peer(2)];
    let launch = launch_of_slot("with", &peers);
    let ids: Vec<_> = launch
        .start
        .unwrap()
        .initial_peers
        .iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(ids, peers.iter().map(|p| p.id).collect::<Vec<_>>());
}

#[test]
fn a_slot_without_the_file_starts_without_peers() {
    let launch = launch_of_slot("without", &[]);
    assert!(launch.start.unwrap().initial_peers.is_empty());
}

#[test]
fn a_slot_start_carries_its_settings_path() {
    let launch = launch_of_slot("path", &[peer(1)]);
    let expected = launch.run.file.as_ref().map(|f| f.path().to_path_buf());
    assert!(expected.is_some());
    assert_eq!(launch.start.unwrap().invited_peers_file, expected);
}

#[test]
fn a_slot_without_a_file_carries_no_path() {
    let mut launch = launch_of_slot("nofile", &[]);
    launch.run.file = None;
    let parsed = slot_key(&launch.run).expect("key");
    let relaunch = launch.relaunch.clone();
    let again = slot_launch(launch.id.clone(), launch.run, parsed, &relaunch);
    assert_eq!(again.start.unwrap().invited_peers_file, None);
}
