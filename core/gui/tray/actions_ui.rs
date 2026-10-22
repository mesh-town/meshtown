//! Tray action items under the exits submenu: "Invite a member" and the checkbox "Allow sites to open via this computer" on top of muda. It knows nothing about the
//! node or HTTP — only about the ready label (`InviteNote`) and availability from `ExitMenu`.
//! Links: `Core::attach_ui` builds `ActionsUi` and inserts it through `menu_build::build_menu`;
//! `Core::apply_exits` calls `apply_exit_menu`, `TrayCmd::Invite` calls `show_invite`.

use tray_icon::menu::{CheckMenuItem, Menu, MenuItem};

use super::core::{MENU_INVITE, MENU_SERVE};
use crate::gui::tray::app::exit_menu::ExitMenu;
use crate::gui::tray::app::tray_actions::{invite_item_text, InviteNote};
use crate::gui::tray::app::tray_texts::SERVE_TEXT;

/// Live items of the actions block.
pub(super) struct ActionsUi {
    invite_item: MenuItem,
    serve: CheckMenuItem,
}

impl ActionsUi {
    /// The invite item with a label by state; unavailable until the first snapshot.
    pub(super) fn build(note: InviteNote) -> ActionsUi {
        ActionsUi {
            invite_item: MenuItem::with_id(MENU_INVITE, invite_item_text(note), false, None),
            serve: CheckMenuItem::with_id(MENU_SERVE, SERVE_TEXT, false, false, None),
        }
    }

    /// Puts the items into the menu.
    pub(super) fn append_to(&self, menu: &Menu) -> Result<(), String> {
        menu.append(&self.invite_item).map_err(|e| e.to_string())?;
        menu.append(&self.serve).map_err(|e| e.to_string())
    }

    /// Availability is the same as for the exits submenu; the checkbox follows the snapshot.
    pub(super) fn apply_exit_menu(&self, menu: &ExitMenu) {
        self.invite_item.set_enabled(menu.enabled);
        self.serve.set_enabled(menu.enabled);
        self.serve.set_checked(menu.serves_exit);
    }

    /// Only the label of the invite item.
    pub(super) fn show_invite(&self, note: InviteNote) {
        self.invite_item.set_text(invite_item_text(note));
    }
}
