//! The "ban vote in progress" menu item: inserted into the live menu while a vote runs, removed after.
//!
//! What it does: keeps the `Menu` handle and the current item, `apply` shows, retexts or removes it.
//! What it does not do: it decides nothing — the text (or its absence) comes from `app::members_note`.
//! Links: `Core::attach_ui` builds `VoteUi`; `Core::refresh_ui` calls `apply`; a click is `MENU_VOTE` in `core_menu`.

use tray_icon::menu::{Menu, MenuItem};

use crate::gui::tray::app::menu_order::vote_position;

/// Identifier of the "ban vote" item.
pub(crate) const MENU_VOTE: &str = "vote";

pub(super) struct VoteUi {
    menu: Menu,
    item: Option<MenuItem>,
}

impl VoteUi {
    pub(super) fn new(menu: Menu) -> VoteUi {
        VoteUi { menu, item: None }
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
        let item = MenuItem::with_id(MENU_VOTE, text, true, None);
        match self.menu.insert(&item, vote_position(notice_ids)) {
            Ok(()) => self.item = Some(item),
            Err(e) => super::log_tray(&format!("vote item: {e}")),
        }
    }

    fn hide(&mut self) {
        if let Some(item) = self.item.take() {
            let _ = self.menu.remove(&item);
        }
    }
}
