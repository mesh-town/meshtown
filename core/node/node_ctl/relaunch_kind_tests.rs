use super::*;
use crate::members::network_key::{key_with_label, managed_key_with_label, NetworkKey};
use crate::transport::net::NodeSecret;

fn test_relaunch() -> Relaunch {
    Relaunch {
        file: None,
        cli: CliOverrides::default(),
        secret: NodeSecret::from_bytes(&[6u8; 32]),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: Some(crate::transport::net::mem::process_net()),
    }
}

#[test]
fn a_managed_key_keeps_the_list_counted_on_the_test_network() {
    // given: a relaunch on the in-process test network, which opens the list for plain nets
    let relaunch = test_relaunch();
    let creator = NodeSecret::from_bytes(&[7u8; 32]).peer_id();
    let key = managed_key_with_label(None, &[3u8; 32], &creator);
    // when: the node is reloaded with a Managed key
    let (_, cfg) = relaunch
        .reload(&key, &Settings::default())
        .expect("reload a managed key");
    // then: the list is counted, so the roles of the members reach the node
    assert!(!cfg.open_list, "a managed network must count its list");
    assert!(cfg.net_kind.root().is_some());
}

#[test]
fn a_peer_key_leaves_the_list_open_on_the_test_network() {
    // given: the same relaunch
    let relaunch = test_relaunch();
    let key = key_with_label(None, &NetworkKey::from_bytes([5u8; 32]));
    // when: the node is reloaded with a plain key
    let (_, cfg) = relaunch
        .reload(&key, &Settings::default())
        .expect("reload a plain key");
    // then: nothing changes for it
    assert!(cfg.open_list);
}
