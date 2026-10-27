//! What `forget_network` does to the node id the slot remembers for the snapshot: leaving a network
//! drops it, so the page does not show the id of the network that was left.
//! Mounted from `lifecycle_tests.rs`.

use super::*;
use crate::node::NodeConfig;
use crate::web_gui::NodeStatus;

#[tokio::test]
async fn forget_network_clears_the_remembered_id() {
    // given: a node that was attached and detached; its id is still in the snapshot
    let mut settings = Settings::default();
    let ctl = NodeCtl::new(&settings);
    let node = crate::node::start_node(NodeConfig::for_loopback_test(
        NetworkKey::generate().unwrap(),
    ))
    .await
    .unwrap();
    ctl.attach(&node);
    ctl.detach(NodeStatus::Starting);
    assert_eq!(
        ctl.snapshot().node.id_full,
        node.id.to_string(),
        "precondition: the id outlives the detach"
    );

    // when: the node leaves the network
    let secret = NodeSecret::from_bytes(&[33u8; 32]);
    forget_network(&mut settings, &ctl, &test_relaunch(), secret);

    // then
    let snap = ctl.snapshot();
    assert_eq!(snap.node.id, "");
    assert_eq!(snap.node.id_full, "");
    node.shutdown().await;
}
