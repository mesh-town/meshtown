//! Marks of an overloaded exit in the "Exit via…" list and the status line.
//!
//! What it does: turns a ready item caption into the marked one (a chosen busy exit gets the orange sign in front,
//! an unchosen overloaded one a tail) and picks the `RouteLine` variant of the active exit.
//! What it does not do: it reads no snapshot and touches no menu — `exit_menu` passes the flags in.
//! Links: `exit_menu` calls these functions while it builds the items and the route line.

use super::tray_texts::{BUSY_EXIT_MARK, OVERLOADED_TAIL};
use super::view::RouteLine;

/// A chosen exit: an online one that says it is overloaded gets the orange sign; an offline one keeps its own mark.
pub(super) fn chosen_label(caption: String, online: bool, busy: bool) -> String {
    match online && busy {
        true => format!("{BUSY_EXIT_MARK} {caption}"),
        false => caption,
    }
}

/// An unchosen member's exit: an online one that is overloaded gets a tail.
pub(super) fn peer_label(caption: String, online: bool, overloaded: bool) -> String {
    match online && overloaded {
        true => format!("{caption}{OVERLOADED_TAIL}"),
        false => caption,
    }
}

/// The line of the online active exit: `ViaBusy` when it is overloaded, else `relayed` picks `ViaRelay`.
pub(super) fn via_variant(name: String, busy: bool, relayed: bool) -> RouteLine {
    match (busy, relayed) {
        (true, _) => RouteLine::ViaBusy(name),
        (false, true) => RouteLine::ViaRelay(name),
        (false, false) => RouteLine::Via(name),
    }
}
