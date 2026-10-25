//! Handling a click on a menu item (`Core::handle_menu`): moved out of `core.rs` for file size.
//! A child module of `core` — it sees the private fields of `Core`.

use super::super::nets_click::{link_for, node_cmd_for, parse_menu_id, MenuTarget};
use super::{log_tray, start_enabled, stop_enabled, Core, NodeCmd};

impl Core {
    /// `TrayCmd::Menu`: a click by item identifier.
    pub(super) fn handle_menu(&mut self, id: &str) {
        let target = parse_menu_id(id);
        match target {
            MenuTarget::Quit => self.quit.request(),
            MenuTarget::Node => self.click_node(),
            // The status line, the proxy line, a network header or an unknown id — silently.
            MenuTarget::Unknown => {}
            MenuTarget::Open
            | MenuTarget::Vote
            | MenuTarget::Add
            | MenuTarget::Notice(_)
            | MenuTarget::Members(_) => self.open_page(&target),
            MenuTarget::Exit { .. } | MenuTarget::Directly => {
                self.apply_click(&target);
                self.send_node_cmds(&target);
            }
            _ => self.send_node_cmds(&target),
        }
    }

    /// Shows the check marks the node will have after the click, before its snapshot arrives.
    fn apply_click(&mut self, target: &MenuTarget) {
        let Some(next) = self.last_exit.as_ref().map(|m| m.after_click(target)) else {
            return;
        };
        self.apply_exits(next);
    }

    /// Opens the page at the place the target stands for.
    fn open_page(&self, target: &MenuTarget) {
        // `None` — a race with the menu item state, not an error: silently.
        let Some(base) = (self.page_url_fn)() else {
            return;
        };
        let Some(url) = link_for(target, &base, &self.facts) else {
            return;
        };
        if let Err(e) = self.tray.open_url(&url) {
            log_tray(&format!("browser did not open: {e}"));
        }
    }

    /// Sends what the click means to the node; an invite click also remembers the network it was for.
    fn send_node_cmds(&mut self, target: &MenuTarget) {
        if let MenuTarget::Invite(slot) = target {
            self.invite_slot = slot.clone().or_else(|| self.flat_slot.clone());
        }
        let cmds = node_cmd_for(
            target,
            self.flat_slot.as_deref(),
            self.serves_exit,
            &self.facts,
        );
        for cmd in cmds {
            let _ = self.node_cmd_tx.send(cmd);
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
}

#[cfg(test)]
#[path = "core_menu_tests.rs"]
mod tests;
