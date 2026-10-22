//! The texts of the "Invite a member" tray items: labels and failure reasons. Pure
//! strings without muda and without HTTP — read by `tray::actions_ui` (the item label) and the `tray_role` poller
//! (the reasons). The invite string never gets here: the reason is only a ready constant.

use crate::gui::tray::app::start_note::{start_note_line, StartNote};
use crate::gui::tray::app::tray_texts::{
    invite_copy_failed_text, serve_disable_failed_text, serve_enable_failed_text,
    INVITE_COPIED_ISSUER_AND_ONE_LINE, INVITE_COPIED_ISSUER_ONLY_LINE, INVITE_COPIED_LINE,
    INVITE_COPIED_TEXT, INVITE_TEXT,
};
use crate::gui::tray::app::view::{node_status_menu_line, RouteLine, View};

/// How many nodes will let a newcomer in with this invite (the `reach` field of the node's reply).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InviteReach {
    Spread,
    IssuerAndOne,
    IssuerOnly,
}

impl InviteReach {
    /// The value of the `reach` field; no field, `spread` and an unfamiliar value — no warning.
    pub(crate) fn from_wire(wire: Option<&str>) -> InviteReach {
        match wire {
            Some("issuer_only") => InviteReach::IssuerOnly,
            Some("issuer_and_one") => InviteReach::IssuerAndOne,
            _ => InviteReach::Spread,
        }
    }

    fn line(self) -> &'static str {
        match self {
            InviteReach::Spread => INVITE_COPIED_LINE,
            InviteReach::IssuerAndOne => INVITE_COPIED_ISSUER_AND_ONE_LINE,
            InviteReach::IssuerOnly => INVITE_COPIED_ISSUER_ONLY_LINE,
        }
    }
}

/// What the invite item shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InviteNote {
    Idle,
    Copied(InviteReach),
    Failed(&'static str),
}

/// The note of the "Allow opening sites via this computer" checkbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ServeNote {
    Idle,
    Failed {
        was_enabling: bool,
        reason: &'static str,
    },
}

/// The item label by state.
pub(crate) fn invite_item_text(note: InviteNote) -> String {
    match note {
        InviteNote::Idle => INVITE_TEXT.to_string(),
        InviteNote::Copied(_) => INVITE_COPIED_TEXT.to_string(),
        InviteNote::Failed(reason) => invite_copy_failed_text(reason),
    }
}

/// The first menu line by the invite note; `None` — there is no note.
pub(crate) fn invite_status_line(note: InviteNote) -> Option<String> {
    match note {
        InviteNote::Idle => None,
        InviteNote::Copied(reach) => Some(reach.line().to_string()),
        InviteNote::Failed(reason) => Some(invite_copy_failed_text(reason)),
    }
}

/// The first menu line by the checkbox note; `None` — there is no note.
pub(crate) fn serve_status_line(note: ServeNote) -> Option<String> {
    match note {
        ServeNote::Idle => None,
        ServeNote::Failed {
            was_enabling: true,
            reason,
        } => Some(serve_enable_failed_text(reason)),
        ServeNote::Failed {
            was_enabling: false,
            reason,
        } => Some(serve_disable_failed_text(reason)),
    }
}

/// The first menu line and the icon hint: the invite note, the checkbox note, the node's refusal
/// reason, otherwise the usual status line.
pub(crate) fn menu_first_line(
    view: View,
    route: &RouteLine,
    invite: InviteNote,
    serve: ServeNote,
    start: &StartNote,
) -> String {
    invite_status_line(invite)
        .or_else(|| serve_status_line(serve))
        .or_else(|| start_note_line(start))
        .unwrap_or_else(|| node_status_menu_line(view, route))
}

#[cfg(test)]
#[path = "tray_actions_tests.rs"]
mod tests;
