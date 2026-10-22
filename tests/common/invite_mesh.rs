//! Helpers for the tests of handing an invite to holders (`tests/invite_holders.rs`): several nodes
//! in one `mem` network, a source for `invite::make`, an issue "like the console". Included via
//! `#[path]` next to `invite_nodes.rs`.
#![allow(dead_code)] // each test crate takes its own subset

use meshtown::invite::{self, Dealt, ParsedInvite, Source};
use meshtown::network_key::NetworkKey;
use meshtown::node::RunningNode;

use super::common::{addr_of, start, wait_members};

/// `n` nodes of one network; every pair is introduced to each other and waits until both become members.
pub(crate) async fn mesh(key: &NetworkKey, n: usize) -> Vec<RunningNode> {
    let mut nodes = Vec::new();
    for _ in 0..n {
        nodes.push(start(key).await);
    }
    for i in 0..n {
        for j in (i + 1)..n {
            nodes[i].introduce(addr_of(&nodes[j])).await;
            wait_members(&nodes[i], &nodes[j]).await;
        }
    }
    nodes
}

/// What `RunningNode::invite_source` (it is `pub(crate)`) assembles from the node's public fields.
pub(crate) fn source_of(node: &RunningNode) -> Source {
    Source {
        stack: node.stack.clone(),
        members: node.members.clone(),
        key: node.network_key.clone(),
        label: node.network_label.clone(),
        domains: node.domains.clone(),
    }
}

/// An issue like the console and the page: `make` → `commit`; together with the parsed string.
pub(crate) async fn deal(node: &RunningNode) -> (Dealt, ParsedInvite) {
    let issued = invite::make(&source_of(node), None).expect("make must assemble a string");
    let dealt = issued.commit().await.expect("commit must write the issue");
    let parsed = invite::parse(&dealt.text).expect("the issued string must parse");
    (dealt, parsed)
}
