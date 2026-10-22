//! Handling a click on a menu item (`Core::handle_menu`): moved out of `core.rs` for file size.
//! A child module of `core` — it sees the private fields of `Core`.

use super::super::vote_item::MENU_VOTE;
use super::{
    log_tray, start_enabled, stop_enabled, Core, NodeCmd, MENU_EXIT_PREFIX, MENU_INVITE, MENU_NODE,
    MENU_OPEN, MENU_QUIT, MENU_SERVE,
};
use crate::gui::tray::app::members_note;

impl Core {
    /// `TrayCmd::Menu`: a click by item identifier.
    pub(super) fn handle_menu(&mut self, id: &str) {
        if id == MENU_OPEN {
            self.click_open();
        } else if id == MENU_VOTE {
            self.click_vote();
        } else if id == MENU_NODE {
            self.click_node();
        } else if id == MENU_INVITE {
            let _ = self.node_cmd_tx.send(NodeCmd::CopyInvite);
        } else if id == MENU_SERVE {
            let _ = self
                .node_cmd_tx
                .send(NodeCmd::SetExitServer(!self.serves_exit));
        } else if id == MENU_QUIT {
            self.quit.request();
        } else if let Some(rest) = id.strip_prefix(MENU_EXIT_PREFIX) {
            self.click_exit(rest);
        }
        // MENU_STATUS, MENU_PROXY and an unknown id — silently (informational items / defensive).
    }

    fn click_open(&self) {
        // `None` — a race with the menu item state, not an error: silently.
        let Some(url) = (self.page_url_fn)() else {
            return;
        };
        if let Err(e) = self.tray.open_url(&url) {
            log_tray(&format!("browser did not open: {e}"));
        }
    }

    /// The vote item opens the same page at the members section.
    fn click_vote(&self) {
        let Some(url) = (self.page_url_fn)() else {
            return;
        };
        if let Err(e) = self
            .tray
            .open_url(&members_note::with_anchor(&url, "peers"))
        {
            log_tray(&format!("browser did not open: {e}"));
        }
    }

    fn click_node(&self) {
        // The poller has already exited — nobody needs the command: silently. The node is moving to
        // Starting/Stopping — the command is not sent.
        if start_enabled(self.view.node) {
            let _ = self.node_cmd_tx.send(NodeCmd::Start);
        } else if stop_enabled(self.view.node) {
            let _ = self.node_cmd_tx.send(NodeCmd::Stop);
        }
    }

    /// An empty tail is "As usual" itself: clear the whole choice, not toggle an empty id.
    fn click_exit(&self, rest: &str) {
        let cmd = match rest.is_empty() {
            true => NodeCmd::ClearExits,
            false => NodeCmd::ToggleExit(rest.to_string()),
        };
        let _ = self.node_cmd_tx.send(cmd);
    }
}
