//! The nested network menu on top of `muda`: a submenu per network in the main menu and the live rows both the
//! network submenus and "Exit via…" are made of. The rows themselves come from `app::nets_rows`; this file only
//! applies them. A submenu is rebuilt only when the shape of the menu changes, otherwise its texts are updated in place.

use tray_icon::menu::{
    CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu, TextStyle,
};

use crate::gui::tray::app::exit_menu::ExitMenu;
use crate::gui::tray::app::menu_order::net_menu_id;
use crate::gui::tray::app::nets_menu::NetMenu;
use crate::gui::tray::app::nets_rows::{layout_key, net_rows, RowKind, RowSpec};

use super::actions_ui::ActionsUi;
use super::core::MENU_NODE;

/// One live row of a submenu.
pub(super) enum LiveRow {
    Item(MenuItem),
    Check(CheckMenuItem),
    Separator(PredefinedMenuItem),
}

impl LiveRow {
    pub(super) fn build(spec: &RowSpec) -> LiveRow {
        let row = match spec.kind {
            RowKind::Separator => LiveRow::Separator(PredefinedMenuItem::separator()),
            RowKind::Check => LiveRow::Check(CheckMenuItem::with_id(
                spec.id.as_str(),
                &spec.text,
                spec.enabled,
                spec.checked,
                None,
            )),
            RowKind::Item | RowKind::Header => LiveRow::Item(MenuItem::with_id(
                spec.id.as_str(),
                &spec.text,
                spec.enabled,
                None,
            )),
        };
        row.update(spec);
        row
    }

    /// Puts the text, state and style of `spec` on the live row.
    pub(super) fn update(&self, spec: &RowSpec) {
        let style = match spec.secondary {
            true => TextStyle::Secondary,
            false => TextStyle::Default,
        };
        match self {
            LiveRow::Separator(_) => {}
            LiveRow::Check(item) => {
                item.set_enabled(spec.enabled);
                item.set_checked(spec.checked);
                item.set_styled_text([(spec.text.as_str(), style)]);
            }
            LiveRow::Item(item) => {
                item.set_enabled(spec.enabled);
                item.set_styled_text([(spec.text.as_str(), style)]);
            }
        }
    }

    pub(super) fn append_to(&self, submenu: &Submenu) -> Result<(), String> {
        let appended = match self {
            LiveRow::Item(item) => submenu.append(item),
            LiveRow::Check(item) => submenu.append(item),
            LiveRow::Separator(item) => submenu.append(item),
        };
        appended.map_err(|e| e.to_string())
    }

    pub(super) fn remove_from(&self, submenu: &Submenu) {
        let _ = match self {
            LiveRow::Item(item) => submenu.remove(item),
            LiveRow::Check(item) => submenu.remove(item),
            LiveRow::Separator(item) => submenu.remove(item),
        };
    }
}

/// The index just before the separator that precedes the node item: where network submenus and the flat actions go.
pub(super) fn insert_position(menu: &Menu) -> usize {
    let at = menu.items().iter().position(is_node_item);
    at.map_or(menu.items().len(), |i| i.saturating_sub(1))
}

fn is_node_item(kind: &MenuItemKind) -> bool {
    kind.id().0 == MENU_NODE
}

struct NetSub {
    submenu: Submenu,
    rows: Vec<LiveRow>,
}

/// The network submenus of the main menu.
pub(super) struct NetsUi {
    menu: Menu,
    subs: Vec<NetSub>,
    key: String,
    /// The networks last applied, to redraw the invite row when only its note changes.
    nets: Vec<NetMenu>,
}

impl NetsUi {
    pub(super) fn new(menu: Menu) -> NetsUi {
        NetsUi {
            menu,
            subs: Vec::new(),
            key: String::new(),
            nets: Vec::new(),
        }
    }

    /// Shows the submenus for two or more networks (and hides the flat invite and checkbox), otherwise removes them.
    /// `note` is the invite row text of one network after a copy: `(slot, text)`.
    pub(super) fn apply(
        &mut self,
        menu: &ExitMenu,
        note: Option<(&str, &str)>,
        actions: &mut ActionsUi,
    ) {
        if menu.nets.len() < 2 {
            self.remove_all();
            actions.set_flat_visible(true);
            return;
        }
        actions.set_flat_visible(false);
        self.nets = menu.nets.clone();
        let key = layout_key(menu);
        match key == self.key {
            true => self.update_in_place(&menu.nets, note),
            false => self.rebuild(&menu.nets, note, key),
        }
    }

    /// Redraws the invite rows after a note changed.
    pub(super) fn show_note(&mut self, note: Option<(&str, &str)>) {
        let nets = self.nets.clone();
        self.update_in_place(&nets, note);
    }

    fn remove_all(&mut self) {
        for sub in self.subs.drain(..) {
            let _ = self.menu.remove(&sub.submenu);
        }
        self.key.clear();
        self.nets.clear();
    }

    fn rebuild(&mut self, nets: &[NetMenu], note: Option<(&str, &str)>, key: String) {
        self.remove_all();
        for net in nets {
            match self.insert_net(net, note_for(net, note)) {
                Ok(sub) => self.subs.push(sub),
                Err(e) => super::log_tray(&format!("network submenu: {e}")),
            }
        }
        self.key = key;
    }

    fn insert_net(&self, net: &NetMenu, note: Option<&str>) -> Result<NetSub, String> {
        let submenu = Submenu::with_id(net_menu_id(&net.slot), &net.label, true);
        let mut rows = Vec::new();
        for spec in net_rows(net, note) {
            let row = LiveRow::build(&spec);
            row.append_to(&submenu)?;
            rows.push(row);
        }
        let at = insert_position(&self.menu);
        self.menu.insert(&submenu, at).map_err(|e| e.to_string())?;
        Ok(NetSub { submenu, rows })
    }

    fn update_in_place(&mut self, nets: &[NetMenu], note: Option<(&str, &str)>) {
        for (sub, net) in self.subs.iter().zip(nets) {
            sub.submenu.set_text(&net.label);
            let specs = net_rows(net, note_for(net, note));
            for (row, spec) in sub.rows.iter().zip(&specs) {
                row.update(spec);
            }
        }
    }
}

fn note_for<'a>(net: &NetMenu, note: Option<(&str, &'a str)>) -> Option<&'a str> {
    note.filter(|(slot, _)| *slot == net.slot)
        .map(|(_, text)| text)
}
