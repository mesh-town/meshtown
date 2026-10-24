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
pub(super) fn admins_of(v: &Verdicts) -> Vec<String> {
    v.ledger.admins.iter().map(|p| p.to_string()).collect()
}

/// The only administrator may not leave: the network would stay without one.
fn is_last_admin(v: &Verdicts) -> bool {
    v.ledger.is_admin(&v.me) && v.ledger.admins.len() == 1
}

/// The flags for this node: in a peer network everyone may invite, remove and leave; in a managed one
/// the administrators may do everything but leave as the last one.
pub fn abilities(v: &Verdicts) -> Abilities {
    if !is_managed(v) {
        return Abilities {
            can_invite: true,
            can_remove: true,
            can_set_admins: false,
            can_delete_network: false,
            can_leave: true,
        };
    }
    let admin = v.ledger.is_admin(&v.me);
    Abilities {
        can_invite: admin,
        can_remove: admin,
        can_set_admins: admin,
        can_delete_network: admin,
        can_leave: !is_last_admin(v),
    }
}

#[cfg(test)]
#[path = "abilities_tests.rs"]
mod tests;
