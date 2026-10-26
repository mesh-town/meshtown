use super::*;
use crate::config::config_file::{self, MainFile};
use crate::transport::net::NodeSecret;

fn test_secret(byte: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[byte; 32])
}

/// Saves a settings file with the old key and reloads it with a new key and cli junk;
/// checks that the key argument beats the file and cli, and the rest comes from the file.
fn assert_reload_with_file_merges_cli_and_key(dir: &std::path::Path, new_key_text: &str) {
    let file = dir.join("config.json");

    let file_settings = Settings {
        net: crate::config::config_file::NetSettings {
            network_key: Some(crate::node::host::test_keys::managed_text(1)),
            exit_enabled: true,
            node_name: Some("from-file".to_string()),
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    config_file::save_config_file(&file, None, 1_790_000_000, &test_secret(9), &file_settings)
        .expect("save");

    let cli = CliOverrides {
        exit_enabled: Some(false),
        network_key: Some("garbage-not-a-key".to_string()),
        ..CliOverrides::default()
    };

    let relaunch = Relaunch {
        file: Some(Arc::new(MainFile(file.clone()))),
        cli,
        secret: test_secret(3),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let prev = Settings::default();
    let (s, cfg) = relaunch
        .reload(new_key_text, &prev)
        .expect("reload with a valid key");
    assert_eq!(
        s.net.network_key.as_deref(),
        Some(new_key_text),
        "network_key = the argument, cli junk does not win"
    );
    assert!(
        !s.net.exit_enabled,
        "cli.exit_enabled=Some(false) must beat the file (true)"
    );
    assert_eq!(s.net.node_name.as_deref(), Some("from-file"));
    assert_eq!(cfg.settings.node_name.as_deref(), Some("from-file"));
    let new_key = network_key::parse_user_input(new_key_text).unwrap().key;
    assert_eq!(
        cfg.network_key.subkeys().network_id_hex(),
        new_key.subkeys().network_id_hex()
    );
}

/// Without a settings file, reload takes values from the previous configuration, and junk instead of a key is
/// an error.
fn assert_reload_without_path_keeps_prev_and_rejects_garbage(new_key_text: &str) {
    let prev2 = Settings {
        net: crate::config::config_file::NetSettings {
            node_name: Some("prev-name".to_string()),
            ..crate::config::config_file::NetSettings::default()
        },
        ..Settings::default()
    };
    let relaunch_no_path = Relaunch {
        file: None,
        cli: CliOverrides::default(),
        secret: test_secret(4),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let (s2, cfg2) = relaunch_no_path
        .reload(new_key_text, &prev2)
        .expect("reload without a file");
    assert_eq!(s2.net.node_name.as_deref(), Some("prev-name"));
    assert_eq!(cfg2.settings.node_name.as_deref(), Some("prev-name"));

    assert!(relaunch_no_path.reload("not a key at all", &prev2).is_err());
}

#[test]
fn relaunch_reload_merges_file_cli_and_new_key() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("mesh-node-ctl-test-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    let new_key_text = crate::node::host::test_keys::managed_text(2u8);

    assert_reload_with_file_merges_cli_and_key(&dir, &new_key_text);
    assert_reload_without_path_keeps_prev_and_rejects_garbage(&new_key_text);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn node_config_sets_peers_cache_beside_config_path() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-node-ctl-cache-test-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    let file = dir.join("config.json");

    let relaunch = Relaunch {
        file: Some(Arc::new(MainFile(file.clone()))),
        cli: CliOverrides::default(),
        secret: test_secret(7),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let key = crate::node::host::test_keys::key_input(8);
    let cfg = relaunch.node_config(&Settings::default(), key);
    assert_eq!(
        cfg.peers_cache_path,
        Some(dir.join("peers-cache.json")),
        "a config path exists — the cache is next to it"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn node_config_leaves_peers_cache_none_without_path() {
    let relaunch = Relaunch {
        file: None,
        cli: CliOverrides::default(),
        secret: test_secret(8),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let key = crate::node::host::test_keys::key_input(9);
    let cfg = relaunch.node_config(&Settings::default(), key);
    assert_eq!(
        cfg.peers_cache_path, None,
        "no config path — the cache is off"
    );
}

#[test]
fn node_config_sets_peer_names_beside_config_path() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-node-ctl-peer-names-test-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    let file = dir.join("config.json");

    let relaunch = Relaunch {
        file: Some(Arc::new(MainFile(file.clone()))),
        cli: CliOverrides::default(),
        secret: test_secret(14),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let key = crate::node::host::test_keys::key_input(15);
    let cfg = relaunch.node_config(&Settings::default(), key);
    assert_eq!(
        cfg.peer_names_path,
        Some(dir.join("peer-names.json")),
        "a config path exists — the names file is next to it"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn node_config_leaves_peer_names_none_without_path() {
    let relaunch = Relaunch {
        file: None,
        cli: CliOverrides::default(),
        secret: test_secret(16),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let key = crate::node::host::test_keys::key_input(17);
    let cfg = relaunch.node_config(&Settings::default(), key);
    assert_eq!(
        cfg.peer_names_path, None,
        "no config path — the names file is off"
    );
}

#[test]
fn node_config_sets_docs_store_beside_config_path() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-node-ctl-docs-store-test-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    let file = dir.join("config.json");

    let relaunch = Relaunch {
        file: Some(Arc::new(MainFile(file.clone()))),
        cli: CliOverrides::default(),
        secret: test_secret(10),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let key = crate::node::host::test_keys::key_input(11);
    let cfg = relaunch.node_config(&Settings::default(), key);
    assert_eq!(
        cfg.docs_store_dir,
        Some(dir.join("docs-store")),
        "a config path exists — the document store is next to it"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn node_config_leaves_docs_store_none_without_path() {
    let relaunch = Relaunch {
        file: None,
        cli: CliOverrides::default(),
        secret: test_secret(12),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let key = crate::node::host::test_keys::key_input(13);
    let cfg = relaunch.node_config(&Settings::default(), key);
    assert_eq!(
        cfg.docs_store_dir, None,
        "no config path — the store is off"
    );
}

#[test]
fn node_config_sets_dht_table_beside_config_path() {
    // given: a main config file in its own folder
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-node-ctl-dht-table-test-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    let relaunch = Relaunch {
        file: Some(Arc::new(MainFile(dir.join("config.json")))),
        cli: CliOverrides::default(),
        secret: test_secret(18),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    // when: the node config is built
    let cfg = relaunch.node_config(
        &Settings::default(),
        crate::node::host::test_keys::key_input(19),
    );
    // then: the DHT table file is next to the config file
    assert_eq!(
        cfg.dht_table_path,
        Some(dir.join("dht-nodes.txt")),
        "a config path exists — the DHT table is next to it"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn node_config_leaves_dht_table_none_without_path() {
    // given: no settings file
    let relaunch = Relaunch {
        file: None,
        cli: CliOverrides::default(),
        secret: test_secret(20),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    // when: the node config is built
    let cfg = relaunch.node_config(
        &Settings::default(),
        crate::node::host::test_keys::key_input(21),
    );
    // then: the DHT table is off
    assert_eq!(
        cfg.dht_table_path, None,
        "no config path — the DHT table is off"
    );
}

#[test]
fn relaunch_reload_keeps_label() {
    let relaunch = Relaunch {
        file: None,
        cli: CliOverrides::default(),
        secret: test_secret(6),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: None,
    };
    let prev = Settings::default();

    let root = test_secret(6).peer_id();
    let with_label = network_key::managed_key_with_label(Some("our-game"), &[5u8; 32], &root);
    let (_, cfg) = relaunch
        .reload(&with_label, &prev)
        .expect("reload with a label");
    assert_eq!(cfg.network_label, Some("our-game".to_string()));

    let without_label = network_key::managed_key_with_label(None, &[5u8; 32], &root);
    let (_, cfg2) = relaunch
        .reload(&without_label, &prev)
        .expect("reload without a label");
    assert_eq!(cfg2.network_label, None);
}

#[test]
fn node_config_with_test_net_runs_on_mem() {
    // given: Relaunch with the tests' mem network
    let net = crate::transport::net::mem::process_net();
    let relaunch = Relaunch {
        file: None,
        cli: CliOverrides::default(),
        secret: test_secret(30),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: Some(net.clone()),
    };
    // when: build the node config
    let cfg = relaunch.node_config(
        &Settings::default(),
        crate::node::host::test_keys::key_input(31),
    );
    // then: the mem transport and the same Arc of the network
    assert_eq!(cfg.settings.net_choice.transports, vec!["mem"]);
    assert!(std::sync::Arc::ptr_eq(cfg.test_net.as_ref().unwrap(), &net));
}

#[path = "relaunch_slot_tests.rs"]
mod net_slot_file;
