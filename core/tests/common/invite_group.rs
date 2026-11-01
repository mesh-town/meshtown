//! Several nodes in one Managed network, for the test of handing an invite to holders. Included via `#[path]`
//! next to `invite_mesh.rs`, whose name it takes as `super::mesh`.

use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::members::access::record::{ensure, AclKind, AclValue, Via};
use meshtown_core::node::RunningNode;
use meshtown_core::transport::net::{NodeSecret, PeerId};
use meshtown_core::transport::net_mainline::unix_now;

use super::common::{addr_of, wait_members};
use super::managed::{wait_can_invite, ManagedNet};

/// The permanent keys of the members of a mesh are `MEMBER_SEED + 1`, `MEMBER_SEED + 2`, …
const MEMBER_SEED: u8 = 40;

/// The creator of the Managed network and `n - 1` members it admitted and made admins (only admins hold
/// invites); every pair is introduced to each other
/// and waits until both become members. The creator is the first node.
pub(crate) async fn mesh(net: &ManagedNet, n: usize) -> Vec<RunningNode> {
    let mut nodes = vec![net.start_root(net.base.clone()).await];
    wait_can_invite(&nodes[0]).await;
    let seeds: Vec<u8> = (1..n as u8).map(|i| MEMBER_SEED + i).collect();
    for seed in &seeds {
        admit(&nodes[0], NodeSecret::from_bytes(&[*seed; 32]).peer_id()).await;
    }
    for seed in &seeds {
        nodes.push(net.start_member(*seed, Vec::new()).await);
    }
    for i in 0..n {
        for j in (i + 1)..n {
            nodes[i].introduce(addr_of(&nodes[j])).await;
            wait_members(&nodes[i], &nodes[j]).await;
        }
    }
    for member in &nodes[1..] {
        let granted = apply(&nodes[0].access, MemberAction::Grant(member.id)).await;
        assert_eq!(granted, Ok(true), "the creator must make a member an admin");
        wait_can_invite(member).await;
    }
    nodes
}

/// The creator writes its admission of `who` into the shared list.
async fn admit(root: &RunningNode, who: PeerId) {
    let value = AclValue {
        kind: AclKind::Admit,
        subject: who,
        via: Via::None,
        made_unix: unix_now(),
        note: String::new(),
    };
    ensure(root.domains.shared_replica().as_ref(), &value)
        .await
        .unwrap();
    root.access.refresh().await.unwrap();
}
