//! What this node may do in its network, for the page and the tray.
//!
//! What it does: reads the verdicts once and answers the administrators of the network and the
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

/// The administrators as full ids in ascending order.
pub fn admins_of(v: &Verdicts) -> Vec<String> {
    v.ledger.admins.iter().map(|p| p.to_string()).collect()
}

/// The flags for this node: the administrators may do everything but leave as the last one.
pub fn abilities(v: &Verdicts) -> Abilities {
    abilities_of(v.ledger.is_admin(&v.me), v.ledger.admins.len())
}

/// The same flags from the facts alone: whether this node is an administrator and how many administrators
/// there are. The only administrator may not leave: the network would stay without one.
pub fn abilities_of(me_admin: bool, admin_count: usize) -> Abilities {
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
