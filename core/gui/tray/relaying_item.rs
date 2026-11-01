//! The "relaying for other members" menu line: inserted right under the status line while this node's relay
//! carries a pair of other members, removed after. It is inactive: a label, not a command.
//!
//! What it does: keeps the `Menu` handle and the current item, `apply` shows, retexts or removes it.
//! What it does not do: it decides nothing — the text (or its absence) comes from `app::tray_texts`.
//! Links: `Core::attach_ui` builds `RelayingUi`; `Core::refresh_ui` calls `apply`; the other inserted items
//! (`admit_item`, `extras_ui`) ask `shown` to place themselves after it.

use tray_icon::menu::{Menu, MenuItem};

/// Identifier of the relaying line.
pub(crate) const MENU_RELAYING: &str = "relaying";

/// Position of the line: right after the status line.
const AT: usize = 1;

pub(super) struct RelayingUi {
    menu: Menu,
    item: Option<MenuItem>,
}

/// Whether the live menu holds the relaying line.
pub(super) fn shown(menu: &Menu) -> bool {
    menu.items().iter().any(|i| i.id().0 == MENU_RELAYING)
}

impl RelayingUi {
    pub(super) fn new(menu: Menu) -> RelayingUi {
        RelayingUi { menu, item: None }
    }

    /// `Some(text)` — the line is present with this text; `None` — absent.
    pub(super) fn apply(&mut self, text: Option<&str>) {
        match (text, self.item.as_ref()) {
            (Some(text), Some(item)) => item.set_text(text),
            (Some(text), None) => self.show(text),
            (None, Some(_)) => self.hide(),
            (None, None) => {}
        }
    }

    fn show(&mut self, text: &str) {
        let item = MenuItem::with_id(MENU_RELAYING, text, false, None);
        match self.menu.insert(&item, AT) {
            Ok(()) => self.item = Some(item),
            Err(e) => super::log_tray(&format!("relaying item: {e}")),
        }
    }

    fn hide(&mut self) {
        if let Some(item) = self.item.take() {
            let _ = self.menu.remove(&item);
        }
    }
}
