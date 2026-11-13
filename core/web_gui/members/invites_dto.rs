//! JSON shape of the open-invite list (`GET /api/network/invites` and the reply of the label edit).

use serde::Serialize;

/// The invites that can still be redeemed, newest first. Shown to admins only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InvitesReply {
    pub invites: Vec<InviteRow>,
}

/// One open invite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InviteRow {
    /// Hash of the secret: 64 hex characters; names the invite in the label edit.
    pub hash: String,
    /// The note for whom the invite was made; `null` when there is none.
    pub for_whom: Option<String>,
    pub made_unix: u64,
    pub expires_unix: u64,
    /// The admin who made the invite (full id); `null` when the record does not name one.
    pub issuer: Option<String>,
    pub issuer_name: Option<String>,
    /// This node made the invite, so it may edit the note.
    pub mine: bool,
    /// The strings of this invite are kept on this node, so it can be copied or shown as a QR code again.
    pub kept: bool,
}

/// The strings of one open invite: the full one and the shorter one for the QR code. A secret: not in `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct InviteText {
    pub text: String,
    pub qr_text: String,
}

impl std::fmt::Debug for InviteText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InviteText").finish_non_exhaustive()
    }
}
