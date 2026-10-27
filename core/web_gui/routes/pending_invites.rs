//! Which slot is already waiting for the invite to a network, so that a repeated invite goes back to it.
//! The node reports no network id for a slot that has not joined yet, so the page handler keeps the pairs.

use crate::config::config_file::NetSlotId;
use crate::web_gui::net_brief::briefs;
use crate::web_gui::App;

/// Notes that the invite to the network `id_hex` was sent into `slot`.
pub(super) fn remember(app: &App, id_hex: &str, slot: NetSlotId) {
    let mut map = app
        .pending_invites
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.insert(id_hex.to_string(), slot);
}

/// Drops the note for the network `id_hex`.
pub(super) fn forget(app: &App, id_hex: &str) {
    let mut map = app
        .pending_invites
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.remove(id_hex);
}

/// The slot that still waits for the invite to the network `id_hex`. A note whose slot is gone or no longer
/// waits (joined, refused, handed back) is dropped.
pub(super) fn waiting_slot(app: &App, id_hex: &str) -> Option<NetSlotId> {
    let noted = {
        let map = app
            .pending_invites
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.get(id_hex).cloned()
    }?;
    if is_waiting(app, &noted) {
        return Some(noted);
    }
    forget(app, id_hex);
    None
}

fn is_waiting(app: &App, slot: &NetSlotId) -> bool {
    let name = slot.to_string();
    app.host.slot(slot).is_some()
        && briefs(app.host.as_ref())
            .iter()
            .any(|b| b.id == name && b.joining)
}
