//! What this node may do in its network, for the page and the tray.
//!
//! What it does: reads the verdicts once and answers the kind of the network, its administrators and the
//! flags of the actions that are open to this node.
//! What it doesn't do: no checks of a concrete action (`actions` does them), no HTTP shapes.

use super::view::Verdicts;

/// The actions open to this node; the page shows no control whose flag is off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Abilities {
    pub can_invite: bool,
    pub can_remove: bool,
    pub can_set_admins: bool,
    pub can_delete_network: bool,
    pub can_leave: bool,
}

/// An administered network always has at least the creator among its administrators.
fn is_managed(v: &Verdicts) -> bool {
    !v.ledger.admins.is_empty()
}

/// `"Managed"` for a network with administrators, `"Peer"` otherwise.
pub(super) fn kind_of(v: &Verdicts) -> &'static str {
    match is_managed(v) {
        true => "Managed",
        false => "Peer",
    }
}

/// The administrators as full ids in ascending order.
pub fn admins_of(v: &Verdicts) -> Vec<String> {
    v.ledger.admins.iter().map(|p| p.to_string()).collect()
}

/// The flags for this node: in a peer network everyone may invite, remove and leave; in a managed one
/// the administrators may do everything but leave as the last one.
pub fn abilities(v: &Verdicts) -> Abilities {
    abilities_of(
        is_managed(v),
        v.ledger.is_admin(&v.me),
        v.ledger.admins.len(),
    )
}

/// The same flags from the facts alone: the kind of the network, whether this node is an administrator and
/// how many administrators there are. The only administrator may not leave: the network would stay without one.
pub fn abilities_of(managed: bool, me_admin: bool, admin_count: usize) -> Abilities {
    if !managed {
        return Abilities {
            can_invite: true,
            can_remove: true,
            can_set_admins: false,
            can_delete_network: false,
            can_leave: true,
        };
    }
    Abilities {
        can_invite: me_admin,
        can_remove: me_admin,
        can_set_admins: me_admin,
        can_delete_network: me_admin,
        can_leave: !(me_admin && admin_count == 1),
    }
}

#[cfg(test)]
#[path = "abilities_tests.rs"]
mod tests;
