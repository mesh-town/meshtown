use crate::members::network_key::NetworkKey;
use crate::node::NodeConfig;

#[test]
fn loopback_config_shares_process_net_and_uses_mem() {
    // given: two loopback configs
    let a = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    let b = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    // then: transport mem, one network per process
    assert_eq!(a.settings.net_choice.transports, vec!["mem"]);
    assert_eq!(a.settings.net_choice.discovery, vec!["invite", "cache"]);
    assert_eq!(a.settings.net_choice.sync, "mesh");
    let (na, nb) = (a.test_net.as_ref().unwrap(), b.test_net.as_ref().unwrap());
    assert!(std::sync::Arc::ptr_eq(na, nb), "MemNet is not shared");
}
