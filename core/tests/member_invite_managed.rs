//! Invites into a Managed network over loopback: only an admin issues and redeems; a newcomer checks that the
//! answered base belongs to the network named by the invite.
//!
//! What it does: nodes started with a Managed key (derived from a base and the creator's node id) and the
//! membership list on; the creator issues, a newcomer redeems and joins, a plain member tries to issue, a plain
//! member forges an issue record, and a serving node answers with a wrong base.
//! What it does not do: no web layer (the code of the refusal in the page answer is a unit test of `node_ctl`);
//! every wait has a ceiling so a hang fails the test.

use meshtown_core::invite::redeem::RedeemError;
use meshtown_core::invite::store::{issue_record, IssueVerdict, Ledger};
use meshtown_core::invite::{self, InviteHead, InviteSecret, NotAdmin};
use meshtown_core::members::network_key::{NetKind, NetworkKey};
use meshtown_core::node::RunningNode;
use meshtown_core::transport::net::NodeSecret;
use meshtown_core::transport::net_mainline::unix_now;

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/managed_net.rs"]
mod managed;
#[path = "common/invite_mesh.rs"]
mod mesh;

use common::{addr_of, wait_until};
use managed::{
    redeem_as, start_joined, wait_both_members, wait_can_invite, wait_verdict_among, ManagedNet,
    VIEW_LIMIT,
};
use mesh::{deal, source_of};

/// The permanent key seeds of the newcomers.
const FIRST_SEED: u8 = 5;
const SECOND_SEED: u8 = 6;

mod creator_invites {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn newcomer_joins_and_becomes_a_member() {
        // given: the creator is up and can invite
        let net = ManagedNet::new();
        let root = net.start_root(net.base.clone()).await;
        wait_can_invite(&root).await;
        // when: the creator issues an invite and the newcomer redeems it
        let (_dealt, parsed) = deal(&root).await;
        let answer = redeem_as(FIRST_SEED, &parsed.head, &parsed.peers)
            .await
            .expect("the creator's invite must be redeemed");
        // then: the invite names the Managed kind and the answered base gives the network's id
        assert_eq!(
            parsed.head.kind,
            net.kind(),
            "the invite must carry the kind"
        );
        assert_eq!(
            parsed.head.reply_key(&answer).subkeys().network_id,
            net.key.subkeys().network_id,
            "the key derived from the answer must be the network's"
        );
        // and: started with that key the newcomer is a member of both sides
        let newcomer = start_joined(&parsed.head, &answer, FIRST_SEED, parsed.peers.clone()).await;
        wait_both_members(&newcomer, &root).await;
        let owner = NodeSecret::from_bytes(&[FIRST_SEED; 32]).peer_id();
        assert!(
            wait_until(VIEW_LIMIT, || root.members.access().allows(owner)).await,
            "the creator must list the newcomer"
        );
        newcomer.shutdown().await;
        root.shutdown().await;
    }
}

mod plain_member_tries_to_invite {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn make_is_refused_as_not_admin() {
        // given: a member who joined by the creator's invite
        let (_net, root, member) = joined_member().await;
        // control: the creator can issue in the same network
        assert!(
            invite::make(&source_of(&root), None).is_ok(),
            "the creator must be able to issue"
        );
        // when
        let got = invite::make(&source_of(&member), None);
        // then: the refusal that the page answers as 409 `not_admin`
        // assumed: `invite::make` fails with `NotAdmin`, which `node_ctl::invite_wait` turns into
        // `ActionError::Denied { code: "not_admin", .. }`.
        let err = got.err().expect("a plain member must not issue an invite");
        assert!(
            err.downcast_ref::<NotAdmin>().is_some(),
            "expected NotAdmin, got {err:#}"
        );
        member.shutdown().await;
        root.shutdown().await;
    }
}

mod plain_member_writes_the_issue_itself {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn newcomer_is_refused_as_not_from_admin() {
        // given: a member with a modified client writes `invite/<hash>` under its own key,
        // and the creator has seen that record
        let (net, root, member) = joined_member().await;
        let forged = forged_head(&net);
        Ledger::of(&member.domains)
            .issue(&forged.secret.hash_hex(), &issue_record(unix_now(), None))
            .await
            .expect("the forged record is written");
        assert!(
            wait_verdict_among(&root, &forged, &[net.root], IssueVerdict::NotAdmin).await,
            "the creator did not see the forged record as a non-admin issue"
        );
        // when: a newcomer redeems it, asking the creator and the forger
        let peers = [addr_of(&root), addr_of(&member)];
        let got = redeem_as(SECOND_SEED, &forged, &peers).await;
        // then: the final refusal, not a key and not a timeout
        assert_eq!(got.err(), Some(RedeemError::NotFromAdmin));
        member.shutdown().await;
        root.shutdown().await;
    }
}

mod serving_node_swaps_the_base {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn newcomer_is_refused_as_wrong_network() {
        // given: the creator answers redemptions with a base that is not the network's
        let net = ManagedNet::new();
        let lie = NetworkKey::generate().unwrap();
        assert_ne!(lie.to_bytes(), net.base.to_bytes());
        let root = net.start_root(lie).await;
        wait_can_invite(&root).await;
        let (_dealt, parsed) = deal(&root).await;
        // when
        let got = redeem_as(FIRST_SEED, &parsed.head, &parsed.peers).await;
        // then: the id derived from the lie with the creator is not the invite's
        assert_eq!(got.err(), Some(RedeemError::WrongNetwork));
        root.shutdown().await;
    }
}

/// The creator and a plain member that joined by the creator's invite, both listed.
async fn joined_member() -> (ManagedNet, RunningNode, RunningNode) {
    let net = ManagedNet::new();
    let root = net.start_root(net.base.clone()).await;
    wait_can_invite(&root).await;
    let (_dealt, parsed) = deal(&root).await;
    let answer = redeem_as(FIRST_SEED, &parsed.head, &parsed.peers)
        .await
        .expect("the creator's invite must be redeemed");
    let member = start_joined(&parsed.head, &answer, FIRST_SEED, parsed.peers.clone()).await;
    wait_both_members(&member, &root).await;
    (net, root, member)
}

/// A head of the network with a secret that no admin issued.
fn forged_head(net: &ManagedNet) -> InviteHead {
    InviteHead {
        secret: InviteSecret::generate().unwrap(),
        network_id: net.key.subkeys().network_id,
        label: None,
        made_unix: unix_now(),
        kind: NetKind::Managed { root: net.root },
    }
}
