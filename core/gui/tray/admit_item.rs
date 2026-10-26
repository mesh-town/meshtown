//! The "a node is waiting to join" menu item: inserted into the live menu while a request waits, removed after.
//!
//! What it does: keeps the `Menu` handle and the current item, `apply` shows, retexts or removes it.
//! What it does not do: it decides nothing — the text (or its absence) comes from `app::members_note`.
//! Links: `Core::attach_ui` builds `AdmitUi`; `Core::refresh_ui` calls `apply`; a click is `MENU_ADMIT` in `core_menu`.

use tray_icon::menu::{Menu, MenuItem};

use crate::gui::tray::app::menu_order::admit_position;

/// Identifier of the "request to join" item.
pub(crate) const MENU_ADMIT: &str = "admit";

pub(super) struct AdmitUi {
    menu: Menu,
    item: Option<MenuItem>,
}

impl AdmitUi {
    pub(super) fn new(menu: Menu) -> AdmitUi {
        AdmitUi { menu, item: None }
    }

    /// `Some(text)` — the item is present with this text; `None` — absent.
    /// A new item goes after the deleted-network notes `notice_ids`.
    pub(super) fn apply(&mut self, text: Option<&str>, notice_ids: &[u64]) {
        match (text, self.item.as_ref()) {
            (Some(text), Some(item)) => item.set_text(text),
            (Some(text), None) => self.show(text, notice_ids),
            (None, Some(_)) => self.hide(),
            (None, None) => {}
        }
    }

    fn show(&mut self, text: &str, notice_ids: &[u64]) {
        let item = MenuItem::with_id(MENU_ADMIT, text, true, None);
        match self.menu.insert(&item, admit_position(notice_ids)) {
            Ok(()) => self.item = Some(item),
            Err(e) => super::log_tray(&format!("admit item: {e}")),
        }
    }

    fn hide(&mut self) {
        if let Some(item) = self.item.take() {
            let _ = self.menu.remove(&item);
        }
    }
}
