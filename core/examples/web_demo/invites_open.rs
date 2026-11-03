//! Open invites of the demo node: the list behind "Pending invites" and the label edit, with the refusals of a real node.

use meshtown_core::web_gui::{ActionError, InviteRow, InvitesReply, MemberError};

use crate::members::refused;

const TTL_SECS: u64 = 7 * 24 * 3600;
const OTHER_ADMIN: &str = "peer-one";

/// One open invite as the demo keeps it.
pub(crate) struct DemoInvite {
    hash: String,
    for_whom: Option<String>,
    made_unix: u64,
    mine: bool,
}

/// The invites that can still be redeemed, newest first.
pub(crate) struct OpenInvites {
    items: Vec<DemoInvite>,
    next: u64,
}

fn hash_of(n: u64) -> String {
    format!("{:064x}", 0x5eed_0000_0000u64 + n)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn clean(note: Option<&str>) -> Option<String> {
    note.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn not_admin() -> MemberError {
    refused("not_admin", "only admins can do this in this network")
}

fn not_found() -> MemberError {
    MemberError::Action(ActionError::NotFound(
        "this invite has been used or has expired".to_string(),
    ))
}

impl OpenInvites {
    /// Two invites: mine, made for "Anna, laptop", and one of another admin without a note.
    pub(crate) fn seeded() -> Self {
        let now = now_unix();
        let items = vec![
            DemoInvite {
                hash: hash_of(1),
                for_whom: Some("Anna, laptop".to_string()),
                made_unix: now.saturating_sub(2 * 3600),
                mine: true,
            },
            DemoInvite {
                hash: hash_of(2),
                for_whom: None,
                made_unix: now.saturating_sub(26 * 3600),
                mine: false,
            },
        ];
        OpenInvites { items, next: 3 }
    }

    /// An invite made from the "Invite a member" card: mine, with its note.
    pub(crate) fn issue(&mut self, for_whom: Option<&str>) {
        let item = DemoInvite {
            hash: hash_of(self.next),
            for_whom: clean(for_whom),
            made_unix: now_unix(),
            mine: true,
        };
        self.next += 1;
        self.items.insert(0, item);
    }

    /// The list for an administrator; an ordinary member is refused.
    pub(crate) fn list(&self, viewer_admin: bool) -> Result<InvitesReply, MemberError> {
        if !viewer_admin {
            return Err(not_admin());
        }
        Ok(InvitesReply {
            invites: self.items.iter().map(row_of).collect(),
        })
    }

    /// Rewrites the note of my invite; an empty note clears it. The reply is the list after it.
    pub(crate) fn relabel(
        &mut self,
        viewer_admin: bool,
        hash: &str,
        for_whom: Option<&str>,
    ) -> Result<InvitesReply, MemberError> {
        if !viewer_admin {
            return Err(not_admin());
        }
        let item = self
            .items
            .iter_mut()
            .find(|i| i.hash == hash)
            .ok_or_else(not_found)?;
        if !item.mine {
            return Err(refused(
                "not_issuer",
                "only the admin who made this invite can rename it",
            ));
        }
        item.for_whom = clean(for_whom);
        self.list(true)
    }
}

fn row_of(i: &DemoInvite) -> InviteRow {
    InviteRow {
        hash: i.hash.clone(),
        for_whom: i.for_whom.clone(),
        made_unix: i.made_unix,
        expires_unix: i.made_unix + TTL_SECS,
        issuer: Some(crate::members::fake_id(if i.mine { "01" } else { "d4" })),
        issuer_name: (!i.mine).then(|| OTHER_ADMIN.to_string()),
        mine: i.mine,
    }
}

#[cfg(test)]
#[path = "invites_open_tests.rs"]
mod tests;
