use crate::config::config_file::NetSettings;
use crate::members::network_key::{NetworkKey, NetworkKeyInput};
use crate::node::NodeConfig;

fn with_key(key: Option<&str>) -> NetSettings {
    NetSettings {
        network_key: key.map(str::to_string),
        ..NetSettings::default()
    }
}

#[test]
fn debug_hides_network_key() {
    // given: a network key with a secret part
    let n = with_key(Some("label--confidential"));
    let cfg = NodeConfig::for_cli(key_input([5u8; 32]), &n);
    // when: printing {:?} of the settings and of the node config
    let texts = [format!("{n:?}"), format!("{cfg:?}")];
    // then: the secret is absent, the marker is present
    for t in &texts {
        assert!(!t.contains("confidential"), "key was printed: {t}");
        assert!(t.contains("<hidden>"), "no marker: {t}");
    }
}

fn key_input(bytes: [u8; 32]) -> NetworkKeyInput {
    NetworkKeyInput {
        label: None,
        key: NetworkKey::from_bytes(bytes),
        from_words: false,
        base: bytes,
        root: crate::transport::net::NodeSecret::from_bytes(&[7; 32]).peer_id(),
    }
}
