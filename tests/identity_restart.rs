//! Integration tests of the node identity: a persistent secret gives the same `PeerId` after
//! a restart, without a secret every start gives a new one.
//! Also: the node name lives as long as the key; a configured name beats
//! the default name, an invalid one is not taken.
//!
//! What it does: starts and stops a node on loopback twice in a row with the same secret and without one.
//! What it doesn't do: it doesn't check the settings file — only that `NodeConfig::identity_secret`
//! reaches `PeerId` (the file part is the `config_file` tests).
//! Links: exercises `meshtown::node::{start_node, NodeConfig}`,
//! `meshtown::words::default_node_name`.

use meshtown::net::NodeSecret;
use meshtown::network_key::NetworkKey;
use meshtown::node::{self, NodeConfig};
use meshtown::words::default_node_name;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_secret_same_id() {
    let key = NetworkKey::generate().unwrap();
    let secret = NodeSecret::from_bytes(&[7u8; 32]);

    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.identity_secret = Some(secret.clone());
    let first = node::start_node(cfg).await.unwrap();
    let first_id = first.id;
    first.shutdown().await;

    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.identity_secret = Some(secret.clone());
    let second = node::start_node(cfg).await.unwrap();
    let second_id = second.id;
    second.shutdown().await;

    assert_eq!(first_id, second_id, "a restart with the same secret changed the PeerId");
    assert_eq!(first_id, secret.peer_id(), "the PeerId is not equal to the secret's public key");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn none_gives_fresh_id() {
    let key = NetworkKey::generate().unwrap();

    let first = node::start_node(NodeConfig::for_loopback_test(key.clone())).await.unwrap();
    let first_id = first.id;
    first.shutdown().await;

    let second = node::start_node(NodeConfig::for_loopback_test(key)).await.unwrap();
    let second_id = second.id;
    second.shutdown().await;

    assert_ne!(first_id, second_id, "without a secret two starts gave the same PeerId");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn name_survives_restart() {
    let key = NetworkKey::generate().unwrap();
    let secret = NodeSecret::from_bytes(&[8u8; 32]);

    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.identity_secret = Some(secret.clone());
    let first = node::start_node(cfg).await.unwrap();
    let first_name = first.name_at_start.clone();
    first.shutdown().await;

    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.identity_secret = Some(secret.clone());
    let second = node::start_node(cfg).await.unwrap();
    let second_name = second.name_at_start.clone();
    second.shutdown().await;

    assert_eq!(first_name, second_name, "the node name changed after a restart with the same secret");
    assert_eq!(first_name, default_node_name(secret.peer_id().as_bytes()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_name_wins_bad_name_falls_back() {
    let key = NetworkKey::generate().unwrap();

    let mut cfg = NodeConfig::for_loopback_test(key.clone());
    cfg.settings.node_name = Some("  Kitchen  ".to_string());
    let first = node::start_node(cfg).await.unwrap();
    assert_eq!(first.name_at_start, "Kitchen");
    first.shutdown().await;

    let mut cfg = NodeConfig::for_loopback_test(key);
    cfg.settings.node_name = Some("a\nb".to_string());
    let second = node::start_node(cfg).await.unwrap();
    assert_eq!(second.name_at_start, default_node_name(second.id.as_bytes()));
    second.shutdown().await;
}
