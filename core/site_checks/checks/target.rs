//! Where a check is run from: straight from this node or through the exit of a member, and which members' exits
//! take part (`exit_targets`, a pure function).

use std::collections::{HashMap, HashSet};

use crate::transport::net::PeerId;

/// One way to check a domain. The schedule runs each target of a domain on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    /// This node's own internet connection.
    Direct,
    /// Through the exit of this member.
    Exit(PeerId),
}

/// Members whose exit this node checks through: the exit flag is on, the member is online, and it is not this node.
/// The order is fixed (by id), so rounds don't shuffle.
pub fn exit_targets(
    exit_flags: &HashMap<PeerId, bool>,
    online: &HashSet<PeerId>,
    me: PeerId,
) -> Vec<PeerId> {
    let mut out: Vec<PeerId> = exit_flags
        .iter()
        .filter(|(id, on)| **on && **id != me && online.contains(*id))
        .map(|(id, _)| *id)
        .collect();
    out.sort();
    out
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
