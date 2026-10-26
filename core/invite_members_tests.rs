//! Tests of `invite::make` over the members of the node's list: who the string names and with which addresses.

use std::net::SocketAddr;

use crate::transport::net::{AddrHint, NodeSecret, PeerId};

use super::*;

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

#[tokio::test]
async fn make_names_listed_members_without_a_connection() {
    use crate::members::access::tally::{Exclusion, How, Ledger};
    use crate::members::access::view::Verdicts;
    use std::collections::{BTreeMap, BTreeSet};
    // given: a node with no connections, whose list admits B and C and has excluded D
    let cfg = crate::node::NodeConfig::for_loopback_test(
        crate::members::network_key::NetworkKey::generate().unwrap(),
    );
    let node = crate::node::start_node(cfg).await.unwrap();
    let src = node.invite_source();
    let excluded = Exclusion {
        how: How::Votes,
        at_micros: 10,
        voters: Vec::new(),
    };
    let mut ledger = Ledger::empty();
    ledger.admitted = [node.id, id(2), id(3), id(4)].into_iter().collect();
    ledger.excluded = BTreeMap::from([(id(4), excluded)]);
    ledger.admins = BTreeSet::from([node.id]);
    let verdicts = Verdicts {
        me: node.id,
        ledger: std::sync::Arc::new(ledger),
        blocks: BTreeSet::new(),
    };
    src.members.access().publish(verdicts);
    // when: it builds an invite and the string is parsed
    let issued = make(&src, None).unwrap();
    let parsed = parse(&issued.text).unwrap();
    // then: the string names the issuer, B and C, and not D
    let got: BTreeSet<PeerId> = parsed.peers.iter().map(|a| a.id).collect();
    assert_eq!(got, BTreeSet::from([node.id, id(2), id(3)]));
    node.shutdown().await;
}

mod proven_addresses {
    use super::*;
    use crate::transport::net::facade::test_support::{secret, Spy};
    use crate::transport::net::facade::NetStackBuilder;
    use crate::transport::net::mem::MemNet;
    use std::time::Duration;

    #[tokio::test]
    async fn make_carries_the_proven_address_of_a_member() {
        // given: a node that proved an address of B but not of C, both listed as members
        let (node, src) = setup_with_proof(&[2, 3], 2).await;
        // when: it builds an invite and the string is parsed
        let issued = make(&src, None).unwrap();
        let parsed = parse(&issued.text).unwrap();
        // then: B carries the proven address, C stays a key alone
        assert_eq!(hints_of(&parsed, 2), vec![AddrHint::Ip(proven_at())]);
        assert!(hints_of(&parsed, 3).is_empty(), "no proof for C");
        node.shutdown().await;
    }

    #[tokio::test]
    async fn commit_carries_the_proven_address_of_a_member() {
        // given: the same node and an invite made by it
        let (node, src) = setup_with_proof(&[2, 3], 2).await;
        let issued = make(&src, None).unwrap();
        // when: the issue record is written and the final string is assembled
        let dealt = tokio::time::timeout(Duration::from_secs(10), issued.commit())
            .await
            .expect("commit finishes")
            .expect("commit succeeds");
        // then: B carries the proven address in the final string too
        let parsed = parse(&dealt.text).unwrap();
        assert_eq!(hints_of(&parsed, 2), vec![AddrHint::Ip(proven_at())]);
        assert!(hints_of(&parsed, 3).is_empty(), "no proof for C");
        node.shutdown().await;
    }

    #[tokio::test]
    async fn make_keeps_a_member_key_only_without_any_proof() {
        // given: a node that proved nothing, with B listed
        let (node, src) = setup_with_proof(&[2], 9).await;
        // when
        let parsed = parse(&make(&src, None).unwrap().text).unwrap();
        // then
        assert!(hints_of(&parsed, 2).is_empty(), "{:?}", parsed.peers);
        node.shutdown().await;
    }

    fn proven_at() -> SocketAddr {
        "203.0.113.7:4000".parse().unwrap()
    }

    fn hints_of(parsed: &ParsedInvite, n: u8) -> Vec<AddrHint> {
        let found = parsed.peers.iter().find(|p| p.id == id(n));
        found.expect("the member is named").hints.clone()
    }

    /// A running node whose invite source has a stack over a spy that proved `proven_at()` for node `proved`;
    /// the node's list admits the members `listed`.
    async fn setup_with_proof(listed: &[u8], proved: u8) -> (crate::node::RunningNode, Source) {
        use crate::members::access::tally::Ledger;
        use crate::members::access::view::Verdicts;
        use std::collections::BTreeSet;
        let cfg = crate::node::NodeConfig::for_loopback_test(
            crate::members::network_key::NetworkKey::generate().unwrap(),
        );
        let node = crate::node::start_node(cfg).await.unwrap();
        let mut src = node.invite_source();
        let spy = Spy::on(&MemNet::new(), &secret(1)).proving(id(proved), proven_at());
        src.stack = NetStackBuilder::new(secret(1))
            .transport(std::sync::Arc::new(spy))
            .build()
            .unwrap();
        let me = src.stack.me();
        let mut ledger = Ledger::empty();
        ledger.admitted = listed.iter().map(|n| id(*n)).chain([me]).collect();
        ledger.admins = BTreeSet::from([me]);
        src.members.access().publish(Verdicts {
            me,
            ledger: std::sync::Arc::new(ledger),
            blocks: BTreeSet::new(),
        });
        (node, src)
    }
}
