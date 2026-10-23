//! The first menu line and the icon tooltip (`Core::menu_first_line`) and everything that redraws it:
//! `TrayCmd::Invite`/`TrayCmd::Serve`, the shared tail of `apply_update`/`apply_exits`. A child module of
//! `core` — it sees the private fields of `Core`. The invite line does not go here: the notes are constants.

use super::{Core, InviteNote, ServeNote};
use crate::gui::tray::app::members_note;
use crate::gui::tray::app::nets_texts::{
    admit_text, invite_copied_line, invite_copied_text, no_connection_tail, vote_text,
};
use crate::gui::tray::app::start_note::{start_note_hint, StartNote};
use crate::gui::tray::app::tray_actions;
use crate::gui::tray::app::tray_actions::{invite_item_text, InviteReach};
use crate::gui::tray::app::view::{
    node_item_text, page_enabled, page_text, start_enabled, stop_enabled, NodeState,
};

use super::super::menu_build::{picture_for, proxy_item_text};
use crate::gui::tray::app::view_picture::IconPicture;

impl Core {
    /// The key of the picture the tray should show now: the view, the membership facts and the spinner frame.
    pub(super) fn icon_key(&self) -> IconPicture {
        picture_for(self.view, self.members, self.spin.idle())
    }

    /// `TrayCmd::SpinTick`: advances the frame of a live ticker and redraws only the icon, not the menu texts.
    pub(super) fn apply_spin_tick(&mut self, generation: u64) {
        if !self.spin.tick(generation) {
            return;
        }
        let key = self.icon_key();
        if let Some(ui) = &mut self.ui {
            ui.show_icon(key);
        }
    }

    /// The first menu line and the icon tooltip: action notes, otherwise the node state.
    pub(super) fn menu_first_line(&self) -> String {
        let start = self.shown_start_note();
        let usual = tray_actions::menu_first_line(
            self.view,
            &self.route,
            self.invite_note,
            self.serve_note,
            start,
        );
        let line = self.net_invite_line().unwrap_or(usual);
        members_note::excluded_line(self.members, self.view.node).unwrap_or(line)
    }

    /// The label of the network the last invite was for; only when several networks are shown.
    fn invite_net_label(&self) -> Option<&str> {
        let slot = self.invite_slot.as_deref()?;
        let nets = &self.facts.nets;
        let found = nets.iter().find(|n| n.slot == slot);
        found.filter(|_| nets.len() >= 2).map(|n| n.label.as_str())
    }

    /// The first line after copying an invite of a named network.
    fn net_invite_line(&self) -> Option<String> {
        let label = self.invite_net_label()?;
        match self.invite_note {
            InviteNote::Copied(InviteReach::Spread) => Some(invite_copied_line(label)),
            _ => None,
        }
    }

    /// `(slot, text)` of the invite row of a network submenu while a note is shown.
    pub(super) fn net_invite_note(&self) -> Option<(String, String)> {
        let label = self.invite_net_label()?;
        let slot = self.invite_slot.clone()?;
        match self.invite_note {
            InviteNote::Idle => None,
            InviteNote::Copied(_) => Some((slot, invite_copied_text(label))),
            note => Some((slot, invite_item_text(note))),
        }
    }

    /// The text of the vote item: with several networks it names the network the vote or request is in.
    fn vote_item_text(&self) -> Option<String> {
        let plain = members_note::members_item_text(self.members)?;
        let named = self.vote_net_label();
        let text = match named {
            Some((label, true)) => vote_text(label),
            Some((label, false)) => admit_text(label),
            None => plain.to_string(),
        };
        Some(text)
    }

    /// The label of the network the vote item points at, and whether it is a removal vote (otherwise a join request).
    fn vote_net_label(&self) -> Option<(&str, bool)> {
        let facts = &self.facts;
        let slot = facts
            .vote_slot
            .as_deref()
            .filter(|_| facts.nets.len() >= 2)?;
        let net = facts.nets.iter().find(|n| n.slot == slot)?;
        Some((net.label.as_str(), self.removal_vote))
    }

    /// The node's refusal reason is visible only for `Absent`: the note arrives before the view.
    fn shown_start_note(&self) -> &StartNote {
        static IDLE: StartNote = StartNote::Idle;
        match self.view.node {
            NodeState::Absent => &self.start_note,
            _ => &IDLE,
        }
    }

    /// `TrayCmd::StartFail`: remembers the reason and redraws the first line and the tooltip.
    pub(super) fn apply_start_note(&mut self, note: StartNote) {
        self.start_note = note;
        self.refresh_ui();
    }

    /// The icon hint tail naming the networks without a connection (several networks only).
    fn offline_tail(&self) -> String {
        self.offline_nets
            .iter()
            .map(|label| no_connection_tail(label))
            .collect()
    }

    /// `TrayCmd::Invite`: remembers the state, sets the item label and the first line.
    pub(super) fn apply_invite(&mut self, note: InviteNote) {
        self.invite_note = note;
        let net_note = self.net_invite_note();
        if let Some(ui) = &mut self.ui {
            ui.actions.show_invite(note);
            let shown = net_note
                .as_ref()
                .map(|(slot, text)| (slot.as_str(), text.as_str()));
            ui.nets.show_note(shown);
        }
        self.refresh_ui();
    }

    /// `TrayCmd::Serve`: remembers the checkbox note and redraws the first line.
    pub(super) fn apply_serve(&mut self, note: ServeNote) {
        self.serve_note = note;
        self.refresh_ui();
    }

    /// The shared tail of `apply_*`: redraws the icon (if it changed) and updates
    /// the text/availability of the menu items from the last remembered `view`/`route`/`proxy`. Without
    /// a raised icon (before `attach_ui`) — silently.
    pub(super) fn refresh_ui(&mut self) {
        let status_text = self.menu_first_line();
        let tooltip = members_note::tooltip_text(&status_text, self.members) + &self.offline_tail();
        let vote_text = self.vote_item_text();
        let proxy_text = proxy_item_text(&self.proxy);
        let open_text =
            start_note_hint(self.shown_start_note()).unwrap_or_else(|| page_text(self.view));
        let key = self.icon_key();
        let Some(ui) = &mut self.ui else { return };

        ui.show_icon(key);
        let _ = ui.tray_icon_handle.set_tooltip(Some(&tooltip));

        ui.status_item.set_text(&status_text);
        ui.open_item.set_text(&open_text);
        ui.open_item.set_enabled(page_enabled(self.view.page));
        ui.node_item.set_text(node_item_text(self.view.node));
        ui.node_item
            .set_enabled(start_enabled(self.view.node) || stop_enabled(self.view.node));
        ui.proxy_item.set_text(&proxy_text);
        ui.vote.apply(vote_text.as_deref());
    }
}
