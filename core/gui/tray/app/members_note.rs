//! Pure tray decisions from the membership facts (`members.admitting`, `members.me_excluded`).
//!
//! What it does: picks the first menu line for an excluded computer, the text of the "request to join" menu item,
//! the icon hint tail and the page address with an anchor.
//! What it does not do: it touches no menu (that is `tray::admit_item`) and reads no JSON (that is `app::snapshot`).
//! Links: `tray::core_line` and `tray::core_menu` call these functions.

use crate::gui::tray::app::snapshot::TrayMembers;
use crate::gui::tray::app::tray_texts::{
    status_excluded_text, ADMIT_ITEM_TEXT, ADMIT_TOOLTIP_TAIL, PENDING_TOOLTIP_TAIL,
};
use crate::gui::tray::app::view::NodeState;

/// The first menu line when this computer was removed from the network; `None` — the usual line.
/// It matters only for a running node: a stopped one says "stopped" itself.
pub(crate) fn excluded_line(members: TrayMembers, node: NodeState) -> Option<String> {
    let shown = members.me_excluded && node == NodeState::Running;
    shown.then(status_excluded_text)
}

/// The text of the "request to join" item; `None` — the item is absent (nobody waits, or we are excluded already).
pub(crate) fn admit_item_text(admitting: u64, excluded: bool) -> Option<&'static str> {
    let shown = admitting > 0 && !excluded;
    shown.then_some(ADMIT_ITEM_TEXT)
}

/// The text of the one menu item for the members section: a request to join.
pub(crate) fn members_item_text(members: TrayMembers) -> Option<&'static str> {
    admit_item_text(members.admitting, members.me_excluded)
}

/// Whether the tray icon shows the "waiting" mark: a request to join waits for us and we
/// are not excluded.
pub(crate) fn alert_wanted(members: TrayMembers) -> bool {
    members_item_text(members).is_some()
}

/// The icon hint: the first line, plus a tail for each of a request to join and our own waiting.
pub(crate) fn tooltip_text(line: &str, members: TrayMembers) -> String {
    let open = !members.me_excluded;
    let tails = [
        (members.admitting > 0 && open, ADMIT_TOOLTIP_TAIL),
        (members.me_pending && open, PENDING_TOOLTIP_TAIL),
    ];
    tails
        .iter()
        .filter(|(shown, _)| *shown)
        .fold(line.to_string(), |text, (_, tail)| format!("{text}{tail}"))
}

/// The page address pointing at a section: an existing anchor is replaced.
pub(crate) fn with_anchor(url: &str, anchor: &str) -> String {
    let base = url.split_once('#').map_or(url, |(base, _)| base);
    format!("{base}#{anchor}")
}

#[cfg(test)]
#[path = "members_note_tests.rs"]
mod tests;
