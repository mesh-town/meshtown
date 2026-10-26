//! Tests of `invite::make` for a node that is no longer in the network: one that was excluded and one that
//! left on its own makes no invite, while a listed node still does. Child module of
//! `invite_tests.rs`.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::invite::{make, OutOfNetwork};
use crate::members::access::tally::{Exclusion, How, Ledger};
use crate::members::access::view::Verdicts;
use crate::members::network_key::NetworkKey;
use crate::node::{start_node, NodeConfig, RunningNode};
use crate::transport::net::{NodeSecret, PeerId};

const REFUSAL: &str = "no longer in the network";

mod an_excluded_node_makes_no_invite {
    use super::*;

    #[tokio::test]
    async fn when_voted_out() {
        // given: the node knows it was excluded by votes
        let node = given_node(Standing::VotedOut).await;
        // when / then
        then_refused(&node);
        node.shutdown().await;
    }

    #[tokio::test]
    async fn when_it_left_on_its_own() {
        // given: the node knows it left the network
        let node = given_node(Standing::Left).await;
        // when / then
        then_refused(&node);
        node.shutdown().await;
    }
}

mod a_listed_node {
    use super::*;

    #[tokio::test]
    async fn makes_an_invite() {
        // given: the node is listed, its neighbour is listed too
        let node = given_node(Standing::Listed).await;
        // when
        let made = make(&node.invite_source(), None);
        // then: the invite is built
        assert!(made.is_ok(), "a listed node must make an invite");
        node.shutdown().await;
    }
}

// ---- helpers ----

/// What the node's own list says about the node itself.
enum Standing {
    Listed,
    VotedOut,
    Left,
}

/// A running node whose published verdicts put it in the given standing.
async fn given_node(standing: Standing) -> RunningNode {
    let cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    let node = start_node(cfg).await.unwrap();
    let verdicts = verdicts_of(node.id, ledger_of(node.id, standing));
    node.members.access().publish(verdicts);
    node
}

/// A ledger with `me` and one more member, `me` standing as told.
fn ledger_of(me: PeerId, standing: Standing) -> Ledger {
    let mut ledger = Ledger::empty();
    ledger.admitted = [me, neighbour()].into_iter().collect();
    ledger.admins = [me].into_iter().collect();
    match standing {
        Standing::Listed => {}
        Standing::VotedOut => exclude(&mut ledger, me, How::Votes),
        Standing::Left => exclude(&mut ledger, me, How::Left),
    }
    ledger
}

fn exclude(ledger: &mut Ledger, who: PeerId, how: How) {
    let exclusion = Exclusion {
        how,
        at_micros: 10,
        voters: vec![neighbour()],
    };
    ledger.excluded.insert(who, exclusion);
}

fn neighbour() -> PeerId {
    NodeSecret::from_bytes(&[2; 32]).peer_id()
}

fn verdicts_of(me: PeerId, ledger: Ledger) -> Verdicts {
    Verdicts {
        me,
        ledger: Arc::new(ledger),
        blocks: BTreeSet::new(),
    }
}

/// `make` refuses and says that the computer is no longer in the network.
fn then_refused(node: &RunningNode) {
    let Err(error) = make(&node.invite_source(), None) else {
        panic!("a node out of the network must not make an invite");
    };
    assert!(
        error.downcast_ref::<OutOfNetwork>().is_some(),
        "unexpected refusal: {error:#}"
    );
    let text = error.to_string();
    assert!(text.contains(REFUSAL), "unexpected refusal: {text}");
}
