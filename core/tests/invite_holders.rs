//! Handing an invite out to holders on loopback: when issuing, the creator asks live members
//! to store the record, and a newcomer joins through a holder even when the creator is already gone.
//!
//! What it does: checks the number and order of holders in the string, a copy of the record at each holder,
//! joining without the creator, one key for two newcomers at two holders, the winner rule on
//! a foreign claim and the coverage flag with one member and with none.
//! What it doesn't do: it doesn't check DHT, relay, the console and the page; frames and the server are in the unit tests of
//! `invite::*`. A foreign outdated admitter (with no claims) is not modeled here.
//! Links: `meshtown_core::invite::{make, Issued::commit, store}`, `core/tests/common/invite_{nodes,mesh}.rs`.

use std::time::Duration;

use meshtown_core::invite::redeem::RedeemError;
use meshtown_core::invite::store::{IssueVerdict, Ledger};
use meshtown_core::invite::Reach;
use meshtown_core::members::network_key::NetworkKey;
use meshtown_core::node::RunningNode;
use meshtown_core::transport::net::{PeerAddr, PeerId};
use meshtown_core::transport::net_mainline::unix_now;
use tokio::time::sleep;

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/invite_mesh.rs"]
mod mesh;
use common::{
    addr_of, fresh_head, quick_opts, record_issue, redeem_via, short_opts, wait_verdict,
    ENTRY_LIMIT_SECS,
};
use mesh::{deal, mesh};

/// Rounds in the two-newcomer test: the race doesn't show up in every round.
const ROUNDS: u32 = 3;

mod three_nodes_and_the_issuer_gone {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn newcomer_enters_through_a_holder_when_the_issuer_is_gone() {
        // given: three nodes in the network, one of them issued a string
        let key = NetworkKey::generate().unwrap();
        let mut nodes = mesh(&key, 3).await;
        let (dealt, parsed) = deal(&nodes[0]).await;
        assert_eq!(
            dealt.holders.len(),
            2,
            "there must be two holders: {:?}",
            dealt.holders
        );
        assert_eq!(dealt.reach(), Reach::Spread);
        let others = [nodes[1].id, nodes[2].id];
        let first_two: Vec<PeerId> = parsed.peers[..2].iter().map(|p| p.id).collect();
        assert!(
            others.iter().all(|id| first_two.contains(id)),
            "the first two in the string are not holders"
        );
        assert_eq!(
            parsed.peers[2].id, nodes[0].id,
            "the creator is third in the string"
        );
        assert!(
            nodes[1].members.is_member(nodes[2].id),
            "HOLD must not break the peering"
        );
        assert!(nodes[0].members.is_member(nodes[1].id) && nodes[0].members.is_member(nodes[2].id));
        // when: the creator is stopped, the newcomer goes by the addresses from the string
        let issuer = nodes.remove(0);
        issuer.shutdown().await;
        let got = redeem_via(
            &parsed.head,
            &parsed.peers,
            quick_opts(&parsed.head, ENTRY_LIMIT_SECS),
        )
        .await;
        // then
        let got = got.expect("the newcomer must join through a holder");
        assert_eq!(got.encode(), key.encode());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn holder_keeps_its_own_copy() {
        // given
        let key = NetworkKey::generate().unwrap();
        let nodes = mesh(&key, 3).await;
        // when
        let (dealt, parsed) = deal(&nodes[0]).await;
        // then: each holder has its issue record in its document
        let entry = format!("invite/{}", parsed.head.secret.hash_hex());
        assert_eq!(dealt.holders.len(), 2);
        for node in nodes.iter().filter(|n| dealt.holders.contains(&n.id)) {
            let copy = node
                .domains
                .shared_replica()
                .get(node.id, entry.as_bytes())
                .await
                .unwrap();
            assert!(
                copy.is_some(),
                "holder {:?} has no copy of its own record",
                node.id
            );
        }
    }
}

mod two_newcomers {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn two_newcomers_through_two_holders_get_one_key() {
        // given: three nodes, a fresh string on every round
        let key = NetworkKey::generate().unwrap();
        let nodes = mesh(&key, 3).await;
        for round in 0..ROUNDS {
            let (_, parsed) = deal(&nodes[0]).await;
            let head = &parsed.head;
            let (a, b) = (&parsed.peers[0], &parsed.peers[1]);
            // when: two at once, each to its own holder
            let (first, second) = tokio::join!(
                redeem_via(
                    head,
                    std::slice::from_ref(a),
                    quick_opts(head, ENTRY_LIMIT_SECS)
                ),
                redeem_via(
                    head,
                    std::slice::from_ref(b),
                    quick_opts(head, ENTRY_LIMIT_SECS)
                ),
            );
            // then
            let keys = [&first, &second].iter().filter(|r| r.is_ok()).count();
            let used = [&first, &second]
                .iter()
                .filter(|r| matches!(r, Err(RedeemError::Used)))
                .count();
            assert_eq!(
                (keys, used),
                (1, 1),
                "round {round}: {first:?} and {second:?}"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn rival_claim_of_a_smaller_id_holds_the_larger_back() {
        // given: two nodes ordered by id; the issue is recorded at both
        let key = NetworkKey::generate().unwrap();
        let mut nodes = mesh(&key, 2).await;
        nodes.sort_by_key(|n| *n.id.as_bytes());
        let (small, large) = (&nodes[0], &nodes[1]);
        let head = fresh_head(&key, unix_now());
        record_issue(small, &head, unix_now()).await;
        record_issue(large, &head, unix_now()).await;
        // when: the smaller writes a claim, the larger saw it, the newcomer comes to the larger
        let hash = head.secret.hash_hex();
        Ledger::of(&small.domains)
            .claim(&hash, unix_now())
            .await
            .unwrap();
        let seen = seen_claim(large, small.id, &hash).await;
        assert!(
            seen,
            "the smaller one's claim did not reach the larger one within 10 s"
        );
        let got = redeem_via(&head, &[addr_of(large)], short_opts(&head, 4)).await;
        // then
        assert!(
            got.is_err(),
            "the larger one let in despite the smaller one's live claim: {got:?}"
        );
        assert!(
            wait_verdict(large, &head, IssueVerdict::Grant, 1).await,
            "the secret must not be redeemed"
        );
    }

    async fn seen_claim(node: &RunningNode, rival: PeerId, hash: &str) -> bool {
        let ledger = Ledger::of(&node.domains);
        for _ in 0..200 {
            if ledger
                .rival_claims(hash, unix_now())
                .await
                .unwrap()
                .contains(&rival)
            {
                return true;
            }
            sleep(Duration::from_millis(50)).await;
        }
        false
    }
}

mod few_members_online {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn one_member_online_gives_issuer_and_one() {
        // given: the creator and one member
        let key = NetworkKey::generate().unwrap();
        let nodes = mesh(&key, 2).await;
        // when
        let (dealt, parsed) = deal(&nodes[0]).await;
        // then
        assert_eq!(dealt.holders, vec![nodes[1].id]);
        assert_eq!(dealt.reach(), Reach::IssuerAndOne);
        assert_eq!(parsed.peers[0].id, nodes[1].id, "the holder goes first");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn alone_gives_issuer_only_and_issuer_first() {
        // given: the only node of the network
        let key = NetworkKey::generate().unwrap();
        let nodes = mesh(&key, 1).await;
        // when
        let (dealt, parsed) = deal(&nodes[0]).await;
        // then
        assert!(dealt.holders.is_empty());
        assert_eq!(dealt.reach(), Reach::IssuerOnly);
        let first: &PeerAddr = &parsed.peers[0];
        assert_eq!(first.id, nodes[0].id, "the creator is first in the string");
    }
}
