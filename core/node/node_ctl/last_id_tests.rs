//! The node id in the snapshot while no node is attached (`last_id.rs`): the slot remembers the id of the
//! last attached node, so the page keeps showing it between `detach` and the next `attach`.
//! Mounted from `last_id.rs`. The clearing on leaving a network is checked in `lifecycle_forget_tests.rs`.

use crate::config::config_file::Settings;
use crate::members::network_key::NetworkKey;
use crate::node::node_ctl::NodeCtl;
use crate::node::output::short_peer_id;
use crate::node::{NodeConfig, RunningNode};
use crate::web_gui::{NodeControl, NodeStatus};

mod before_the_first_attach {
    use super::*;

    #[tokio::test]
    async fn snapshot_has_no_id_before_the_first_attach() {
        // given
        let ctl = NodeCtl::new(&Settings::default());

        // when
        let snap = ctl.snapshot();

        // then
        assert_eq!(snap.node.id, "");
        assert_eq!(snap.node.id_full, "");
    }
}

mod after_detach {
    use super::*;

    #[tokio::test]
    async fn snapshot_keeps_the_node_id_between_detach_and_attach() {
        // given: a node attached and then detached
        let ctl = NodeCtl::new(&Settings::default());
        let node = start_test_node().await;
        ctl.attach(&node);
        ctl.detach(NodeStatus::Starting);

        // when
        let snap = ctl.snapshot();

        // then: the id of the last node is still there, in both forms
        assert_eq!(snap.node.id_full, node.id.to_string());
        assert_eq!(snap.node.id, short_peer_id(&node.id));
        assert!(!snap.node.id.is_empty(), "the id must not be empty");
        node.shutdown().await;
    }

    #[tokio::test]
    async fn a_new_attach_replaces_the_remembered_id() {
        // given: a node attached and detached, then another node attached
        let ctl = NodeCtl::new(&Settings::default());
        let first = start_test_node().await;
        ctl.attach(&first);
        ctl.detach(NodeStatus::Starting);
        let second = start_test_node().await;
        ctl.attach(&second);
        ctl.detach(NodeStatus::Starting);

        // when
        let snap = ctl.snapshot();

        // then
        assert_eq!(snap.node.id_full, second.id.to_string());
        assert_ne!(snap.node.id_full, first.id.to_string());
        first.shutdown().await;
        second.shutdown().await;
    }
}

async fn start_test_node() -> RunningNode {
    let cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    crate::node::start_node(cfg).await.unwrap()
}
