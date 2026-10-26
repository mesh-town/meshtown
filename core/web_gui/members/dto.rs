//! JSON shapes of `GET /api/members` and of member actions (field order = key order in the reply).

use serde::Serialize;

/// The whole member list as seen by this node.
#[derive(Clone, Debug, Serialize)]
pub struct MembersReply {
    pub me: String,
    pub me_excluded: bool,
    /// This node waits for the confirmation of others and has been turned away for it.
    pub me_pending: bool,
    /// Administrators of the network (full ids).
    pub admins: Vec<String>,
    /// This node is an administrator.
    pub me_admin: bool,
    /// The creator of the network (full id), whose role cannot be taken away; set by `set_root`.
    pub root: Option<String>,
    /// The own request, shown only while `me_pending`.
    pub me_admission: Option<AdmissionInfo>,
    pub counts: Counts,
    pub members: Vec<MemberRow>,
    /// Present only in replies to actions: whether the action changed anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed: Option<bool>,
}

impl MembersReply {
    /// Takes the creator from the key of the network, not from the administrators seen.
    pub fn set_root(&mut self, root: &crate::transport::net::PeerId) {
        self.root = Some(root.to_string());
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Counts {
    pub members: usize,
    pub excluded: usize,
    pub unlisted: usize,
    pub pending: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemberRow {
    pub id: String,
    pub id_full: String,
    pub name: Option<String>,
    #[serde(rename = "self")]
    pub is_self: bool,
    pub online: bool,
    /// The node is an administrator.
    pub admin: bool,
    pub status: &'static str,
    pub excluded: Option<ExcludedInfo>,
    pub blocked_by_me: bool,
    pub denied_by_me: Option<&'static str>,
    /// Seconds since the last sign of life; set by `stamp_last_seen`.
    pub last_seen_sec: Option<u64>,
    pub last_refusal: Option<RefusalInfo>,
    pub admission: Option<AdmissionInfo>,
}

/// A request for the list that waits for an administrator.
#[derive(Clone, Debug, Serialize)]
pub struct AdmissionInfo {
    /// The note of the invitation (for whom it was made); `null` when there is none.
    pub label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExcludedInfo {
    pub how: &'static str,
    pub at_unix: u64,
    pub voters: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RefusalInfo {
    pub side: &'static str,
    pub reason: &'static str,
    pub at_unix: u64,
}
