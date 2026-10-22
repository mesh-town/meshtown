//! The tray submenu "Exit via…": building and updating the live `Submenu` from the ready model
//! `app::exit_menu::ExitMenu`. What it does not do: it knows nothing about HTTP or the node — only about
//! muda and the already-folded model brought by `Core::apply_exits`.
//! Paints an offline item gray and puts a red mark on a chosen offline exit:
//! the text is `ExitItem::menu_text()`, the item color is set by calling `text_style` on the `CheckMenuItem`.
//! Links: `Core::attach_ui` builds `ExitUi` once and inserts `submenu()` into the menu (after
//! "Open"); `Core::apply_exits` (`TrayCmd::ExitMenu`) calls `apply_exit_menu()` on every new snapshot.

use tray_icon::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu, TextStyle};

use crate::app::exit_menu::{ExitItem, ExitMenu};
use crate::app::tray_texts::{direct_text, SUBMENU_TITLE};

use super::core::MENU_EXIT_PREFIX;

/// Text style of an item: an offline node is gray (`Secondary`, muda 0.20 on macOS), otherwise like an
/// ordinary menu item.
fn text_style(item: &ExitItem) -> TextStyle {
    if item.online {
        TextStyle::Default
    } else {
        TextStyle::Secondary
    }
}

/// The container's own id — not `MENU_EXIT_PREFIX` (that one is reserved for the leaf item
/// "As usual"): a click does not open the submenu as an item, but the id must be unique.
const SUBMENU_ID: &str = "exit-menu";

/// The live submenu: the `Submenu` itself, the constant "As usual" item (we change it only with `set_checked`) and
/// the items for other nodes — `apply_exit_menu` rebuilds them wholly on every snapshot (the list is short,
/// a piecewise diff does not pay off). `Submenu::items()` returns `MenuItemKind`, which does not implement
/// `IsMenuItem` — it cannot be fed to `remove`, so we keep our own handles to the items.
pub(super) struct ExitUi {
    submenu: Submenu,
    direct: CheckMenuItem,
    peers: Vec<CheckMenuItem>,
    hint: Option<MenuItem>,
}

impl ExitUi {
    /// An empty submenu: unavailable, only "As usual" and a separator — the contents and availability
    /// are set by the first `apply_exit_menu()` (it comes with the first snapshot).
    pub(super) fn build() -> Result<ExitUi, String> {
        let submenu = Submenu::with_id(SUBMENU_ID, SUBMENU_TITLE, false);
        let direct = CheckMenuItem::with_id(MENU_EXIT_PREFIX, direct_text(), true, true, None);
        submenu.append(&direct).map_err(|e| e.to_string())?;
        submenu
            .append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        Ok(ExitUi {
            submenu,
            direct,
            peers: Vec::new(),
            hint: None,
        })
    }

    /// A reference to the submenu — inserted into the tray menu exactly once, at build time.
    pub(super) fn submenu(&self) -> &Submenu {
        &self.submenu
    }

    /// Rebuilds the list from a new snapshot: the container's availability comes from
    /// `menu.enabled`; "As usual" is checked when `menu.items` has no chosen item; the list of other
    /// nodes — old items removed, new ones added at the end (after "As usual" and the separator).
    pub(super) fn apply_exit_menu(&mut self, menu: ExitMenu) {
        self.submenu.set_enabled(menu.enabled);
        self.direct
            .set_checked(menu.items.iter().all(|item| !item.checked));
        for old in self.peers.drain(..) {
            let _ = self.submenu.remove(&old);
        }
        for item in &menu.items {
            let id = format!("{MENU_EXIT_PREFIX}{}", item.id_full);
            let check = CheckMenuItem::with_id(id, item.menu_text(), true, item.checked, None);
            check.set_styled_text([(item.menu_text(), text_style(item))]);
            match self.submenu.append(&check) {
                Ok(()) => self.peers.push(check),
                Err(e) => super::log_tray(&format!("exits submenu: {e}")),
            }
        }
        self.update_hint(&menu);
    }

    /// Removes the previous hint and, if there are no exits, sets an inactive one after the separator.
    fn update_hint(&mut self, menu: &ExitMenu) {
        if let Some(old) = self.hint.take() {
            let _ = self.submenu.remove(&old);
        }
        let Some(text) = menu.empty_hint() else {
            return;
        };
        let hint = MenuItem::new(text, false, None);
        match self.submenu.append(&hint) {
            Ok(()) => self.hint = Some(hint),
            Err(e) => super::log_tray(&format!("exits submenu: {e}")),
        }
    }
}

#[cfg(test)]
#[path = "exit_menu_tests.rs"]
mod tests;
