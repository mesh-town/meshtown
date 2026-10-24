//! Tray action items under the exits submenu: "Invite a member" and the checkbox "Allow sites to open via this computer" on top of muda. It knows nothing about the
//! node or HTTP — only about the ready label (`InviteNote`) and availability from `ExitMenu`.
//! Links: `Core::attach_ui` builds `ActionsUi` and inserts it through `menu_build::build_menu`;
//! `Core::apply_exits` calls `apply_exit_menu`, `TrayCmd::Invite` calls `show_invite`.

use tray_icon::menu::{CheckMenuItem, Menu, MenuItem};

use super::nets_ui::insert_position;

use super::core::{MENU_INVITE, MENU_SERVE};
use crate::gui::tray::app::exit_menu::ExitMenu;
use crate::gui::tray::app::tray_actions::{flat_invite_text, InviteNote};
use crate::gui::tray::app::tray_texts::SERVE_TEXT;

/// Live items of the actions block.
pub(super) struct ActionsUi {
    invite_item: MenuItem,
    serve: CheckMenuItem,
    /// The menu the items were put into, to hide and show them again.
    menu: Option<Menu>,
    flat_visible: bool,
    /// The last note and right to invite: together they decide the label of the invite item.
    note: InviteNote,
    can_invite: bool,
}

impl ActionsUi {
    /// The invite item with a label by state; unavailable until the first snapshot.
    pub(super) fn build(note: InviteNote) -> ActionsUi {
        ActionsUi {
            invite_item: MenuItem::with_id(MENU_INVITE, flat_invite_text(true, note), false, None),
            serve: CheckMenuItem::with_id(MENU_SERVE, SERVE_TEXT, false, false, None),
            menu: None,
            flat_visible: true,
            note,
            can_invite: true,
        }
    }

    /// Puts the items into the menu.
    pub(super) fn append_to(&mut self, menu: &Menu) -> Result<(), String> {
        self.menu = Some(menu.clone());
        menu.append(&self.invite_item).map_err(|e| e.to_string())?;
        menu.append(&self.serve).map_err(|e| e.to_string())
    }

    /// Availability is the same as for the exits submenu; the checkbox follows the snapshot.
    pub(super) fn apply_exit_menu(&mut self, menu: &ExitMenu) {
        self.can_invite = menu.can_invite;
        self.invite_item
            .set_text(flat_invite_text(self.can_invite, self.note));
        self.invite_item
            .set_enabled(menu.enabled && menu.can_invite);
        self.serve.set_enabled(menu.enabled);
        self.serve.set_checked(menu.serves_exit);
    }

    /// With two or more networks the invite and the checkbox live in the network submenus, so the flat ones leave the
    /// menu; they come back at the same place.
    pub(super) fn set_flat_visible(&mut self, visible: bool) {
        let Some(menu) = &self.menu else { return };
        if visible == self.flat_visible {
            return;
        }
        self.flat_visible = visible;
        if !visible {
            let _ = menu.remove(&self.invite_item);
            let _ = menu.remove(&self.serve);
            return;
        }
        let at = insert_position(menu);
        let _ = menu.insert(&self.invite_item, at);
        let _ = menu.insert(&self.serve, at + 1);
    }

    /// Only the label of the invite item.
    pub(super) fn show_invite(&mut self, note: InviteNote) {
        self.note = note;
        self.invite_item
            .set_text(flat_invite_text(self.can_invite, note));
    }
}
