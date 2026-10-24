//! Menu items that come and go with the node's state, on top of `muda`: a note per deleted network right after
//! "Open Mesh Town…" and "Add a network…" after the network block. The order comes from `app::menu_order`; this file
//! only inserts and removes the widgets.

use tray_icon::menu::{Menu, MenuItem};

use crate::gui::tray::app::exit_menu::ExitMenu;
use crate::gui::tray::app::menu_order::{has_network, middle_ids, notice_id, top_ids};
use crate::gui::tray::app::nets_texts::net_deleted_text;
use crate::gui::tray::app::overview::Notice;
use crate::gui::tray::app::tray_texts::ADD_NET_TEXT;

use super::nets_ui::insert_position;

/// Identifier of the "Add a network…" item.
pub(crate) const MENU_ADD: &str = "add";

pub(super) struct ExtrasUi {
    menu: Menu,
    notes: Vec<(Notice, MenuItem)>,
    add: Option<MenuItem>,
}

impl ExtrasUi {
    pub(super) fn new(menu: Menu) -> ExtrasUi {
        ExtrasUi {
            menu,
            notes: Vec::new(),
            add: None,
        }
    }

    /// The numbers of the notes shown now, in menu order.
    pub(super) fn note_ids(&self) -> Vec<u64> {
        self.notes.iter().map(|(n, _)| n.id).collect()
    }

    /// Shows one note per deleted network; nothing happens while the list is the same.
    pub(super) fn apply_notices(&mut self, notices: &[Notice]) {
        if self.notes.iter().map(|(n, _)| n).eq(notices) {
            return;
        }
        for (_, item) in self.notes.drain(..) {
            let _ = self.menu.remove(&item);
        }
        let ids: Vec<u64> = notices.iter().map(|n| n.id).collect();
        let places = top_ids(&ids, false);
        for (note, id) in notices.iter().zip(&ids) {
            let at = places.iter().position(|p| *p == notice_id(*id));
            let item = MenuItem::with_id(notice_id(*id), net_deleted_text(&note.label), true, None);
            match self.menu.insert(&item, at.unwrap_or(2)) {
                Ok(()) => self.notes.push((note.clone(), item)),
                Err(e) => super::log_tray(&format!("deleted network note: {e}")),
            }
        }
    }

    /// Keeps "Add a network…" last in the network block, or removes it when no network is shown. The block must
    /// already be applied; the item moves only when something else was put after it.
    pub(super) fn apply_add(&mut self, menu: &ExitMenu) {
        let wanted = has_network(menu);
        let last = middle_ids(menu, wanted).pop();
        if wanted == self.add.is_some() && (!wanted || self.live_last() == last) {
            return;
        }
        if let Some(item) = self.add.take() {
            let _ = self.menu.remove(&item);
        }
        if wanted {
            self.place_add();
        }
    }

    /// The id of the item just before the separator that precedes the node item.
    fn live_last(&self) -> Option<String> {
        let at = insert_position(&self.menu).checked_sub(1)?;
        Some(self.menu.items().get(at)?.id().0.clone())
    }

    fn place_add(&mut self) {
        let item = MenuItem::with_id(MENU_ADD, ADD_NET_TEXT, true, None);
        match self.menu.insert(&item, insert_position(&self.menu)) {
            Ok(()) => self.add = Some(item),
            Err(e) => super::log_tray(&format!("add item: {e}")),
        }
    }
}
