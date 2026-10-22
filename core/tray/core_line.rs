//! The first menu line and the icon tooltip (`Core::menu_first_line`) and everything that redraws it:
//! `TrayCmd::Invite`/`TrayCmd::Serve`, the shared tail of `apply_update`/`apply_exits`. A child module of
//! `core` — it sees the private fields of `Core`. The invite line does not go here: the notes are constants.

use super::{Core, InviteNote, ServeNote};
use crate::app::members_note;
use crate::app::start_note::{start_note_hint, StartNote};
use crate::app::tray_actions;
use crate::app::view::{
    node_item_text, page_enabled, page_text, start_enabled, stop_enabled, NodeState,
};

use super::super::menu_build::{picture_for, proxy_item_text};
use crate::app::view_picture::IconPicture;

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
        let usual = tray_actions::menu_first_line(self.view, &self.route, self.invite_note, self.serve_note, start);
        members_note::excluded_line(self.members, self.view.node).unwrap_or(usual)
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

    /// `TrayCmd::Invite`: remembers the state, sets the item label and the first line.
    pub(super) fn apply_invite(&mut self, note: InviteNote) {
        self.invite_note = note;
        if let Some(ui) = &self.ui {
            ui.actions.show_invite(note);
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
        let tooltip = members_note::tooltip_text(&status_text, self.members);
        let vote_text = members_note::members_item_text(self.members);
        let proxy_text = proxy_item_text(&self.proxy);
        let open_text = start_note_hint(self.shown_start_note())
            .unwrap_or_else(|| page_text(self.view));
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
        ui.vote.apply(vote_text);
    }
}
