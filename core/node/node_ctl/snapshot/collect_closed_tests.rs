//! The `closed` flag of the members block (`collect.rs`, `overlay_members`): it tells the page that an
//! administrator closed the network to new members. It is read from the published list at every call.
//! Mounted from `collect_tests.rs`.

use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::members::access::tally::{Closed, Ledger};
use crate::members::access::view::Verdicts;
use crate::members::network_key::NetworkKey;
use crate::node::{NodeConfig, RunningNode};
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;
use crate::web_gui::NodeSnapshot;

mod open_network {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlay_members_reports_an_open_network() {
        // given: a published list without a closing record
        let node = loopback_node().await;
        node.access.view.publish(verdicts_of_one(node.id, None));
        let mut snap = NodeSnapshot::default();

        // when
        overlay_members(&mut snap, &node);

        // then
        assert!(
            !snap.members.closed,
            "no closing record: the network is open"
        );
        node.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlay_members_without_a_published_list_is_not_closed() {
        // given: a fresh node that published nothing
        let node = loopback_node().await;
        assert!(
            node.access.view.verdicts().is_none(),
            "precondition: nothing is published"
        );
        let mut snap = NodeSnapshot::default();

        // when
        overlay_members(&mut snap, &node);

        // then
        assert!(!snap.members.closed);
        node.shutdown().await;
    }
}

mod closed_network {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn overlay_members_reports_a_closed_network() {
        // given: a published list whose ledger carries a closing record
        let node = loopback_node().await;
        let closing = Closed {
            by: pid(2),
            at_unix: 5,
        };
        node.access
            .view
            .publish(verdicts_of_one(node.id, Some(closing)));
        let mut snap = NodeSnapshot::default();

        // when
        overlay_members(&mut snap, &node);

        // then
        assert!(
            snap.members.closed,
            "a closing record: the network is closed"
        );
        node.shutdown().await;
    }
}

async fn loopback_node() -> RunningNode {
    let cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    crate::node::start_node(cfg).await.unwrap()
}

fn verdicts_of_one(me: PeerId, closed: Option<Closed>) -> Verdicts {
    Verdicts {
        me,
        ledger: Arc::new(Ledger {
            admitted: BTreeSet::from([me]),
            excluded: BTreeMap::new(),
            admissions: BTreeMap::new(),
            admins: Default::default(),
            closed,
        }),
        blocks: BTreeSet::new(),
    }
}
