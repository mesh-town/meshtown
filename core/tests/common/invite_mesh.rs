//! The invite source of a running node, shared by the Managed-network helpers (`managed_net.rs` takes it as
//! `super::mesh::source_of`). Included via `#[path]` next to `invite_nodes.rs`. Issuing (`deal`) is in
//! `invite_deal.rs` and the multi-node mesh in `invite_group.rs`; a scenario includes only the leaf it needs.

use meshtown_core::invite::Source;
use meshtown_core::node::RunningNode;

/// What `RunningNode::invite_source` (it is `pub(crate)`) assembles from the node's public fields.
pub(crate) fn source_of(node: &RunningNode) -> Source {
    Source {
        stack: node.stack.clone(),
        members: node.members.clone(),
        key: node.network_key.clone(),
        root: node.net_root,
        reply_secret: node.reply_secret.clone(),
        label: node.network_label.clone(),
        domains: node.domains.clone(),
    }
}
