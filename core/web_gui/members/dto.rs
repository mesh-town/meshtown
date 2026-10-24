//! JSON shapes of `GET /api/members` and of member actions (field order = key order in the reply).

use serde::Serialize;

/// The whole member list as seen by this node.
#[derive(Clone, Debug, Serialize)]
pub struct MembersReply {
    pub me: String,
    pub me_excluded: bool,
    /// The members removed this node from the member list.
    pub me_removed: bool,
    /// This node waits for the confirmation of others and has been turned away for it.
    pub me_pending: bool,
    pub need: u8,
    /// `"Peer"` or `"Managed"`.
    pub kind: &'static str,
    /// Administrators of a managed network (full ids); empty in a peer network.
    pub admins: Vec<String>,
    /// This node is an administrator.
    pub me_admin: bool,
    /// The own request, shown only while `me_pending`.
    pub me_admission: Option<AdmissionInfo>,
    pub counts: Counts,
    pub members: Vec<MemberRow>,
    /// Present only in replies to actions: whether the action changed anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Counts {
    pub members: usize,
    pub voting: usize,
    pub excluded: usize,
    pub unlisted: usize,
    pub pending: usize,
    pub forgotten: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemberRow {
    pub id: String,
    pub id_full: String,
    pub name: Option<String>,
    #[serde(rename = "self")]
    pub is_self: bool,
    pub online: bool,
    /// The node is an administrator of a managed network.
    pub admin: bool,
    pub status: &'static str,
    pub vote: Option<VoteInfo>,
    pub excluded: Option<ExcludedInfo>,
    pub blocked_by_me: bool,
    pub denied_by_me: Option<&'static str>,
    pub forget: Option<ForgetInfo>,
    /// Seconds since the last sign of life; set by `stamp_last_seen`.
    pub last_seen_sec: Option<u64>,
    pub last_refusal: Option<RefusalInfo>,
    pub admission: Option<AdmissionInfo>,
}

/// A request for the list that has not got all confirmations it needs.
#[derive(Clone, Debug, Serialize)]
pub struct AdmissionInfo {
    pub votes: usize,
    pub need: u8,
    pub voters: Vec<String>,
    pub mine: bool,
    /// The note of the invitation (for whom it was made); `null` when there is none.
    pub label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VoteInfo {
    pub votes: usize,
    pub need: u8,
    pub proposed_by: String,
    pub voters: Vec<String>,
    pub mine: bool,
    /// The reason noted by the first voter; `null` when there is none or it is not fit to show.
    pub reason: Option<String>,
}

/// A circle of votes to remove a node from the member list.
#[derive(Clone, Debug, Serialize)]
pub struct ForgetInfo {
    pub votes: usize,
    pub need: u8,
    pub proposed_by: String,
    pub voters: Vec<String>,
    pub mine: bool,
    /// When the node was removed; `null` while the circle waits for more votes.
    pub at_unix: Option<u64>,
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
