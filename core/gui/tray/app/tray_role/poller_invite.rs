//! "Invite a member": the poller takes an invite from the node, puts it on the clipboard
//! and shows the outcome in the menu item for `PollTiming.note_visible_for`. The invite string is a secret: it lives
//! only in local variables of this file and goes only into `copy`; it never reaches the log, `Debug`, the error
//! text or the tray channel — only ready-made reason constants go out.

use std::time::Duration;

use serde_json::Value;
use tokio::time::Instant;

use crate::gui::tray::app::local_api::{ApiError, Reply};
use crate::gui::tray::app::snapshot::lenient;
use crate::gui::tray::app::tray_actions::{InviteNote, InviteReach, ServeNote};
use crate::gui::tray::app::tray_texts::{
    REASON_CLIP_FAILED, REASON_JOINING, REASON_NOT_ADMIN, REASON_NO_ANSWER,
};

use super::api::{fetch_reply, NodeRequest};
use super::poll_types::CopyFn;
use super::poller::Poller;

/// Deadline for a clipboard write: the clipboard program may hang.
const CLIP_TIMEOUT: Duration = Duration::from_secs(2);

/// Returns the invite string from the node reply, or the reason for refusal (a ready-made constant).
pub(super) fn invite_from(reply: &Result<Reply, ApiError>) -> Result<String, &'static str> {
    match reply {
        Ok(Reply::Ok(body)) => invite_line_from_body(body).ok_or(REASON_NO_ANSWER),
        Ok(Reply::NotAdmin) => Err(REASON_NOT_ADMIN),
        Ok(Reply::OtherStatus(409)) => Err(REASON_JOINING),
        Ok(Reply::OtherStatus(_))
        | Ok(Reply::Unauthorized)
        | Ok(Reply::Accepted)
        | Ok(Reply::Stopping)
        | Err(ApiError::Refused)
        | Err(ApiError::Timeout)
        | Err(ApiError::NotHttp(_)) => Err(REASON_NO_ANSWER),
    }
}

/// Body of the invite reply. No `Debug`: the invite string is a secret.
#[derive(Default, serde::Deserialize)]
#[serde(default)]
struct InviteBody {
    #[serde(deserialize_with = "lenient::opt_string")]
    invite: Option<String>,
    #[serde(deserialize_with = "lenient::opt_string")]
    reach: Option<String>,
}

/// Returns the invite reach from the node reply; anything but `Ok(Reply::Ok(body))`, or a missing field, gives `Spread`.
pub(super) fn reach_from(reply: &Result<Reply, ApiError>) -> InviteReach {
    match reply {
        Ok(Reply::Ok(body)) => {
            let body: InviteBody = lenient::from_object(body);
            InviteReach::from_wire(body.reach.as_deref())
        }
        _ => InviteReach::Spread,
    }
}

fn invite_line_from_body(body: &Value) -> Option<String> {
    let body: InviteBody = lenient::from_object(body);
    body.invite.filter(|t| !t.is_empty())
}

impl Poller {
    /// A click on the item: request → clipboard write → outcome in the item for `note_visible_for`. `&mut self` is there for the same
    /// reason as in `send_exit_edit`: `Poller` is not `Sync`.
    pub(super) async fn copy_invite(&mut self, slot: Option<&str>) {
        let api = NodeRequest::Invite(slot);
        let reply = fetch_reply(self.port, &self.token_path, api, None).await;
        let text = invite_from(&reply);
        let reach = reach_from(&reply);
        drop(reply);
        let outcome = match text {
            Ok(text) => write_clipboard(self.copy_to_clipboard_fn.clone(), text).await,
            Err(reason) => Err(reason),
        };
        self.show_invite_outcome(outcome, reach);
    }

    /// Shows the outcome and arms the deadline for restoring the label (a repeated click overwrites the deadline).
    fn show_invite_outcome(&mut self, outcome: Result<(), &'static str>, reach: InviteReach) {
        let note = match outcome {
            Ok(()) => InviteNote::Copied(reach),
            Err(reason) => InviteNote::Failed(reason),
        };
        self.say(log_line(outcome));
        (self.send_invite_note)(note);
        self.notes_visible_until = Some(Instant::now() + self.timing.note_visible_for);
    }

    /// The deadline passed: both notes go away.
    pub(super) fn clear_expired_notes(&mut self) {
        self.notes_visible_until = None;
        (self.send_invite_note)(InviteNote::Idle);
        (self.send_serve_note)(ServeNote::Idle);
    }
}

/// Writes through `spawn_blocking` under `CLIP_TIMEOUT`; a timeout or a panic counts as a clipboard refusal. A free
/// function: `&Poller` across `.await` is not `Send` (`Poller` is not `Sync`).
async fn write_clipboard(copy: CopyFn, text: String) -> Result<(), &'static str> {
    let job = crate::node::output::spawn_blocking(move || copy(&text));
    match tokio::time::timeout(CLIP_TIMEOUT, job).await {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(_)) | Err(_) => Err(REASON_CLIP_FAILED),
    }
}

fn log_line(outcome: Result<(), &'static str>) -> String {
    match outcome {
        Ok(()) => "invite: copied".to_string(),
        Err(reason) => format!("invite: couldn't copy — {reason}"),
    }
}

#[cfg(test)]
#[path = "poller_invite_tests.rs"]
mod tests;
