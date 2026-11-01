//! Issuing an invite from a running node, for the tests of handing an invite to holders. Included via `#[path]`
//! by the scenarios that issue invites; it takes the invite source from `invite_mesh.rs` (`super::mesh`).

use meshtown_core::invite::{self, Dealt, ParsedInvite};
use meshtown_core::node::RunningNode;

use super::mesh::source_of;

/// An issue like the console and the page: `make` → `commit`; together with the parsed string.
pub(crate) async fn deal(node: &RunningNode) -> (Dealt, ParsedInvite) {
    let issued = invite::make(&source_of(node), None).expect("make must assemble a string");
    let dealt = issued.commit().await.expect("commit must write the issue");
    let parsed = invite::parse(&dealt.text).expect("the issued string must parse");
    (dealt, parsed)
}
