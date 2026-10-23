//! Editing the list of selected exits from a tray submenu command: the pure `edit_exits` plus
//! the round `GET /api/config` → edit → `PUT /api/config` with `rev`; a conflict (409, a race with the node or
//! another tab) or a network hiccup between the two requests triggers one more round, and a second
//! failure goes quietly to the log (the next snapshot poll updates the submenu to the fact anyway).

use tokio::time::Instant;

use crate::gui::tray::app::local_api::{self, Reply};
use crate::gui::tray::app::tray_actions::ServeNote;
use crate::gui::tray::app::tray_texts::REASON_SERVE_REFUSED;

use super::config_doc::ConfigDoc;
use super::poller::Poller;

/// An edit command parsed from `NodeCmd::ToggleExit`/`NodeCmd::ClearExits`.
pub(super) enum ExitEdit {
    Toggle(String),
    Clear,
}

/// The pure list edit: `Toggle` makes this exit the only selected one, and pressing it again on the
/// checked one clears the selection (a direct connection remains), like the button on the page. `Clear`
/// empties the list.
pub(super) fn edit_exits(list: &mut Vec<String>, edit: &ExitEdit) {
    match edit {
        ExitEdit::Toggle(id) if list.contains(id) => list.clear(),
        ExitEdit::Toggle(id) => *list = vec![id.clone()],
        ExitEdit::Clear => list.clear(),
    }
}

impl Poller {
    /// One round, and a second one on failure; if both are exhausted, a log line is written and the view is left alone.
    /// Takes `&mut self` although the body changes nothing: `Poller` is not `Sync` (fields `Box<dyn Fn + Send>`),
    /// and holding `&Poller` across `.await` in a future that `spawn` requires to be `Send` is not allowed — the same
    /// trick as in `tick`/`tick_own`/`tick_foreign` nearby.
    pub(super) async fn send_exit_edit(&mut self, slot: Option<&str>, edit: ExitEdit) {
        if self.try_edit_once(slot, &edit).await || self.try_edit_once(slot, &edit).await {
            return;
        }
        self.say("exits: edit was not applied".to_string());
    }

    /// The "Allow sites to open through this computer" checkbox: a `GET` → `PUT` round with `exit.enabled`, and a second round on failure.
    /// Whatever the outcome, the last model is forgotten so that the next tick sends the checkbox again
    /// from the node snapshot (it converges to the fact, not to the click).
    pub(super) async fn send_exit_server(&mut self, slot: Option<&str>, enable: bool) {
        let done =
            self.try_serve_once(slot, enable).await || self.try_serve_once(slot, enable).await;
        self.last_exit_menu = None;
        if done {
            (self.send_serve_note)(ServeNote::Idle);
            return;
        }
        self.say("permission to open sites: edit was not applied".to_string());
        let reason = REASON_SERVE_REFUSED;
        (self.send_serve_note)(ServeNote::Failed {
            was_enabling: enable,
            reason,
        });
        self.notes_visible_until = Some(Instant::now() + self.timing.note_visible_for);
    }

    async fn try_serve_once(&mut self, slot: Option<&str>, enable: bool) -> bool {
        let token = super::run::read_token(&self.token_path);
        let Ok(Reply::Ok(current)) = local_api::get_config(self.port, &token, slot).await else {
            return false;
        };
        let Some(rev) = ConfigDoc::from_value(&current).rev else {
            return false;
        };
        let body = exit_server_body(rev, enable);
        matches!(
            local_api::put_config(self.port, &token, slot, &body).await,
            Ok(Reply::Ok(_))
        )
    }

    /// `GET config` → `edit_exits` on its `route.exits` → `PUT config` with the same `rev`. Any
    /// slip (network, someone else's edit between the requests, a node refusal) yields `false`, without telling the cause.
    /// A deliberate simplification: a retry only on a stale `rev` (409) would be stricter, but here
    /// the whole round is repeated on any failure without parsing the response code — `edit_exits` is idempotent
    /// (the same `Toggle`/`Clear` a second time gives the same result), so an extra retry on a non-409
    /// cause does not corrupt data, it only spends a request.
    async fn try_edit_once(&mut self, slot: Option<&str>, edit: &ExitEdit) -> bool {
        let token = super::run::read_token(&self.token_path);
        let current = match local_api::get_config(self.port, &token, slot).await {
            Ok(Reply::Ok(value)) => value,
            _ => return false,
        };
        let doc = ConfigDoc::from_value(&current);
        let Some(rev) = doc.rev else {
            return false;
        };
        let mut exits = doc.exits();
        edit_exits(&mut exits, edit);
        let body = serde_json::json!({"rev": rev, "config": {"route": {"exits": exits}}})
            .to_string()
            .into_bytes();
        matches!(
            local_api::put_config(self.port, &token, slot, &body).await,
            Ok(Reply::Ok(_))
        )
    }
}

/// Body of `PUT /api/config` for the "Allow sites to open through this computer" checkbox: only `exit.enabled` under `rev`.
pub(super) fn exit_server_body(rev: u64, on: bool) -> Vec<u8> {
    serde_json::json!({"rev": rev, "config": {"exit": {"enabled": on}}})
        .to_string()
        .into_bytes()
}

#[cfg(test)]
#[path = "poller_exit_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "poller_serve_tests.rs"]
mod serve_tests;
