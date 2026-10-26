use super::*;
use crate::members::network_key::parse_user_input;
use crate::members::network_key::test_net;
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
fn reload_counts_the_list_on_the_test_network() {
    // given: a relaunch on the in-process test network
    let relaunch = test_relaunch();
    let net = test_net("Office");
    // when: the node is reloaded with a key
    let (_, cfg) = relaunch
        .reload(&net.text, &Settings::default())
        .expect("reload a key");
    // then: the list is counted, so the roles of the members reach the node
    assert!(
        !cfg.open_list,
        "a network entered at run time must count its list"
    );
    assert_eq!(cfg.net_root, net.root);
}

#[test]
fn a_slot_on_the_test_network_keeps_the_list_open() {
    // given: a relaunch on the in-process test network and a key
    let relaunch = test_relaunch();
    let net = test_net("Office");
    let input = parse_user_input(&net.text).expect("parse");
    // when: the start settings of a slot are built
    let cfg = relaunch.node_config(&Settings::default(), input);
    // then: the test network leaves the list open
    assert!(cfg.open_list);
}
