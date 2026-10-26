//! Who may issue an invite: in a Managed network only an admin, and never a node that left the network.
//!
//! What it does: tells a Peer network (no restriction) from a Managed one and, in the latter, checks that this
//! node is an admin and lists the admins the view lets in (they alone are asked to keep the invite).
//! What it doesn't do: builds no string and writes no record (`invite::make`, `Issued::commit`).
//! Links: called by `invite::make`; the refusal is recognized by `node::node_ctl::invite_wait`.

use std::collections::BTreeSet;
use std::fmt;

use super::Source;
use crate::transport::net::PeerId;

/// The refusal of an issue: this node is not an admin of its Managed network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAdmin;

impl fmt::Display for NotAdmin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("only an admin can invite to this network")
    }
}

impl std::error::Error for NotAdmin {}

/// The refusal of an issue: this node left the network, was excluded or forgotten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfNetwork;

impl fmt::Display for OutOfNetwork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("this computer is no longer in the network, so it cannot invite anyone")
    }
}

impl std::error::Error for OutOfNetwork {}

/// `None` in a Peer network; in a Managed one the listed admins, or `NotAdmin` when this node is not one.
pub(super) fn issuing_admins(src: &Source) -> Result<Option<BTreeSet<PeerId>>, NotAdmin> {
    if src.kind.root().is_none() {
        return Ok(None);
    }
    let verdicts = src.members.access().verdicts().ok_or(NotAdmin)?;
    if !verdicts.ledger.is_admin(&src.stack.me()) {
        return Err(NotAdmin);
    }
    let listed = verdicts
        .ledger
        .admins
        .iter()
        .filter(|p| verdicts.check(**p).is_ok())
        .copied();
    Ok(Some(listed.collect()))
}
