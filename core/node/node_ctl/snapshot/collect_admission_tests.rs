//! The `admission_refused` flag of the members block (`collect.rs`, `overlay_members`): it is read from the live
//! node at every call. With no published list it is down; a stranger that refused this node lifts it, except for
//! the reason "unlisted".
//! Mounted from `collect.rs`.

use super::overlay_members;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::members::access::tally::Ledger;
use crate::members::access::view::{Deny, Side, Verdicts};
use crate::members::network_key::NetworkKey;
use crate::node::{NodeConfig, RunningNode};
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;
use crate::web_gui::NodeSnapshot;

mod without_a_published_list {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlay_without_verdicts_has_no_refusal() {
        // given: a fresh node that published nothing, and an empty snapshot
        let node = loopback_node().await;
        assert!(
            node.access.view.verdicts().is_none(),
            "precondition: nothing is published"
        );
        let mut snap = NodeSnapshot::default();

        // when
        overlay_members(&mut snap, &node);

        // then
        assert!(!snap.members.admission_refused);
        node.shutdown().await;
    }
}

mod a_lone_node_refused_by_a_stranger {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlay_reports_a_refusal_of_a_stranger() {
        // given: a published list of one and a stranger that turned this node away by its own block
        let node = node_refused_by_stranger(Deny::Refused).await;
        let mut snap = NodeSnapshot::default();

        // when
        overlay_members(&mut snap, &node);

        // then
        assert!(snap.members.admission_refused);
        node.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_excluded_refusal_marks_me_excluded_not_refused() {
        // given: nobody is online and a stranger turned this node away as excluded
        let node = node_refused_by_stranger(Deny::Excluded).await;
        let mut snap = NodeSnapshot::default();

        // when
        overlay_members(&mut snap, &node);

        // then: the node knows it is out of the network, which is not a plain refusal
        assert!(
            snap.members.me_excluded,
            "the node must learn it was excluded"
        );
        assert!(!snap.members.admission_refused);
        node.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlay_skips_an_unlisted_refusal() {
        // given: the stranger turned this node away only because it is not on its list
        let node = node_refused_by_stranger(Deny::Unlisted).await;
        let mut snap = NodeSnapshot::default();

        // when
        overlay_members(&mut snap, &node);

        // then
        assert!(!snap.members.admission_refused);
        node.shutdown().await;
    }
}

async fn loopback_node() -> RunningNode {
    let cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    crate::node::start_node(cfg).await.unwrap()
}

/// A node whose list holds only itself and which `pid(9)` refused (the node is not told to forget it on publish:
/// the refusal is noted after the list).
async fn node_refused_by_stranger(deny: Deny) -> RunningNode {
    let node = loopback_node().await;
    node.access.view.publish(list_of_one(node.id));
    node.access.view.note_refusal(pid(9), Side::Them, deny);
    node
}

fn list_of_one(me: PeerId) -> Verdicts {
    Verdicts {
        me,
        ledger: Arc::new(Ledger {
            admitted: BTreeSet::from([me]),
            excluded: BTreeMap::new(),
            admissions: BTreeMap::new(),
            admins: Default::default(),
            closed: None,
        }),
        blocks: BTreeSet::new(),
    }
}
