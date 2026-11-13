//! Building the `exit` part of the snapshot: the interface and the clients.

use std::collections::BTreeSet;

use super::build::Inputs;

/// The page form of an exit interface with its live state.
pub(super) fn exit_iface_info(
    pair: &(String, crate::iface_bind::IfaceState),
) -> crate::iface_bind::ExitIfaceInfo {
    crate::iface_bind::ExitIfaceInfo {
        name: pair.0.clone(),
        state: pair.1,
    }
}

/// Exit clients for `exit.clients`: full ids, network members only, ascending by string, without
/// duplicates; `[]` if the exit is off.
pub(super) fn exit_clients(i: &Inputs<'_>) -> Vec<String> {
    if !i.exit_on {
        return Vec::new();
    }
    let ids: BTreeSet<String> = i
        .exit_clients
        .iter()
        .filter(|id| i.members.contains(*id))
        .map(|id| id.to_string())
        .collect();
    ids.into_iter().collect()
}
