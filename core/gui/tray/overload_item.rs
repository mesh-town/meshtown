//! The "this computer is turning members away" menu item: inserted into the live menu while this computer's own
//! exit is overloaded, removed after.
//!
//! What it does: keeps the `Menu` handle and the current item, `apply` shows, retexts or removes it.
//! What it does not do: it decides nothing — the text comes from `app::members_note`, the place from the caller.
//! Links: `Core::attach_ui` builds `OverloadUi`; `Core::refresh_ui` calls `apply`; a click is `MENU_OVERLOAD`
//! in `core_menu`.

use tray_icon::menu::{Menu, MenuItem};

/// Identifier of the "turning members away" item.
pub(crate) const MENU_OVERLOAD: &str = "overload";

pub(super) struct OverloadUi {
    menu: Menu,
    item: Option<MenuItem>,
}

impl OverloadUi {
    pub(super) fn new(menu: Menu) -> OverloadUi {
        OverloadUi { menu, item: None }
    }

    /// `Some(text)` — the item is present with this text; `None` — absent. A new item goes to `position`.
    pub(super) fn apply(&mut self, text: Option<&str>, position: usize) {
        match (text, self.item.as_ref()) {
            (Some(text), Some(item)) => item.set_text(text),
            (Some(text), None) => self.show(text, position),
            (None, Some(_)) => self.hide(),
            (None, None) => {}
        }
    }

    fn show(&mut self, text: &str, position: usize) {
        let item = MenuItem::with_id(MENU_OVERLOAD, text, true, None);
        match self.menu.insert(&item, position) {
            Ok(()) => self.item = Some(item),
            Err(e) => super::log_tray(&format!("overload item: {e}")),
        }
    }

    fn hide(&mut self) {
        if let Some(item) = self.item.take() {
            let _ = self.menu.remove(&item);
        }
    }
}
