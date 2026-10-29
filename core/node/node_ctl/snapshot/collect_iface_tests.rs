//! `exit_iface_of`: the configured exit interface with its live state, or nothing.

use super::*;

use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::IfaceState;
use crate::members::network_key::NetworkKey;
use crate::node::NodeConfig;

async fn node_with(name: Option<&str>) -> RunningNode {
    let key = NetworkKey::generate().unwrap();
    let node = crate::node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    node.exit_iface.set(name);
    node
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_present_interface_is_up() {
    let name = loopback_name();
    let node = node_with(Some(&name)).await;
    assert_eq!(exit_iface_of(&node), Some((name, IfaceState::Up)));
    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_absent_interface_is_missing() {
    let node = node_with(Some(MISSING_IFACE)).await;
    assert_eq!(
        exit_iface_of(&node),
        Some((MISSING_IFACE.to_string(), IfaceState::Missing))
    );
    node.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_interface_gives_nothing() {
    let node = node_with(None).await;
    assert_eq!(exit_iface_of(&node), None);
    node.shutdown().await;
}
