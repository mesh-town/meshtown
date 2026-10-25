//! The rule "this computer is kept out of the network", computed from the member list for `/api/state`.

use crate::web_gui::{MemberRow, MembersReply};

/// Other members, without the row of this computer.
fn others(reply: &MembersReply) -> impl Iterator<Item = &MemberRow> {
    reply.members.iter().filter(|m| !m.is_self)
}

/// A member that turned this computer away for a reason other than "unlisted".
fn refuses(row: &MemberRow) -> bool {
    row.last_refusal
        .as_ref()
        .is_some_and(|r| r.side == "them" && r.reason != "unlisted")
}

/// `true` when this computer is not excluded, removed or pending, no other member is online, and at least
/// one member refused it. No delay: the page waits before showing it.
pub(in crate::node::node_ctl) fn admission_refused(reply: &MembersReply) -> bool {
    if reply.me_excluded || reply.me_removed || reply.me_pending {
        return false;
    }
    !others(reply).any(|m| m.online) && others(reply).any(refuses)
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
