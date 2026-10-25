//! The optimistic "Exit via…" model after a click: the menu toolkit flips the pressed check mark at once,
//! while the previous choice (and "Directly") would stay marked until the next node snapshot. This
//! brick computes the check marks the way the node will have them after the edit (`edit_exits` in
//! `tray_role::poller_exit` has the same semantics), so the tray can re-apply the model before the node answers.

use crate::gui::tray::app::exit_menu::{ExitItem, ExitMenu};
use crate::gui::tray::nets_click::MenuTarget;

impl ExitMenu {
    /// A copy of the model with the `checked` flags as they will be after the click: a pressed
    /// exit becomes the only checked one in its list, pressing a checked one clears the list, "Directly"
    /// clears every list. Other targets leave the model as it is.
    pub fn after_click(&self, target: &MenuTarget) -> ExitMenu {
        let mut next = self.clone();
        match target {
            MenuTarget::Exit { slot, id } => next.toggle(slot.as_deref(), id),
            MenuTarget::Directly => next.clear_all(),
            _ => {}
        }
        next
    }

    fn toggle(&mut self, slot: Option<&str>, id: &str) {
        if slot.is_none() || slot == self.slot.as_deref() {
            toggle_list(&mut self.items, id);
        }
        for group in self
            .groups
            .iter_mut()
            .filter(|g| Some(g.slot.as_str()) == slot)
        {
            toggle_list(&mut group.items, id);
        }
    }

    fn clear_all(&mut self) {
        clear_list(&mut self.items);
        for group in &mut self.groups {
            clear_list(&mut group.items);
        }
    }
}

/// `Toggle` on one list; an id that is not in the list changes nothing.
fn toggle_list(items: &mut [ExitItem], id: &str) {
    let Some(pressed) = items.iter().find(|i| i.id_full == id) else {
        return;
    };
    let keep = !pressed.checked;
    for item in items.iter_mut() {
        item.checked = keep && item.id_full == id;
    }
}

fn clear_list(items: &mut [ExitItem]) {
    for item in items.iter_mut() {
        item.checked = false;
    }
}

#[cfg(test)]
#[path = "exit_menu_click_tests.rs"]
mod tests;
