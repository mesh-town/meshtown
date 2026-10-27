//! The id of the last attached node, kept in the slot so the snapshot still names it between `detach` and
//! the next `attach`. `attach` records it; leaving a network clears it.

use super::state::NodeCtlState;
use crate::node::output::short_peer_id;
use crate::node::RunningNode;

/// Records the short and full id of `node` as the last attached one.
pub(super) fn remember_id(st: &mut NodeCtlState, node: &RunningNode) {
    st.last_id = Some((short_peer_id(&node.id), node.id.to_string()));
}

/// The remembered id pair, if a node has been attached since the slot was created or the network was left.
pub(super) fn id_pair_of(st: &NodeCtlState) -> Option<(String, String)> {
    st.last_id.clone()
}

#[cfg(test)]
#[path = "last_id_tests.rs"]
mod tests;
