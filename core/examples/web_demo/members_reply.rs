//! `GET /api/members` of the demo node: the list as the page sees it, and the `NodeControl` entry points.

use meshtown_core::web_gui::{
    AdmissionInfo, Counts, ExcludedInfo, MemberActionReq, MemberError, MemberRow, MembersReply,
    NodeStatus, VoteInfo,
};

use meshtown_core::web_gui::ActionError;

use crate::members::{DemoMembers, Entry, Standing, ME};
use crate::members_forget::{forget_info, last_seen};
use crate::node::{DemoNode, DemoState};

/// Confirmations that admit a newcomer, as the node counts them.
const ADMIT_NEED: u8 = 1;

const AT_UNIX: u64 = 1_792_400_000;

/// The demo node's own 64-hex id (`boot_id` followed by zeros), as `/api/state` shows it.
pub(crate) fn demo_self_id_full(boot_id: &str) -> String {
    format!("{boot_id}{}", "0".repeat(64 - boot_id.len()))
}

fn resolve(id: &str, me: &str) -> String {
    if id == ME {
        me.to_string()
    } else {
        id.to_string()
    }
}

fn resolve_all(ids: &[String], me: &str) -> Vec<String> {
    ids.iter().map(|v| resolve(v, me)).collect()
}

fn status_of(e: &Entry) -> &'static str {
    match e.standing {
        Standing::Member => "member",
        Standing::Pending => "pending",
        Standing::Unlisted => "unlisted",
        Standing::Excluded(_) => "excluded",
        Standing::Forgotten => "forgotten",
    }
}

fn denied_by_me(m: &DemoMembers, e: &Entry) -> Option<&'static str> {
    let mine = e.voters.iter().any(|v| v == ME);
    match (&e.standing, e.id_full == ME) {
        (_, true) => None,
        _ if m.me_excluded => Some("left"),
        (Standing::Excluded(_), _) => Some("excluded"),
        _ if mine => Some("voted_by_me"),
        _ if e.blocked => Some("blocked"),
        (Standing::Forgotten, _) => Some("removed"),
        _ if e.forget.voters.iter().any(|v| v == ME) => Some("removed_by_me"),
        (Standing::Unlisted | Standing::Pending, _) => Some("unlisted"),
        _ => None,
    }
}

fn vote_of(m: &DemoMembers, e: &Entry, me: &str) -> Option<VoteInfo> {
    let first = e.voters.first()?;
    Some(VoteInfo {
        votes: e.voters.len(),
        need: m.need(),
        proposed_by: resolve(first, me),
        voters: resolve_all(&e.voters, me),
        mine: e.voters.iter().any(|v| v == ME),
        reason: e.reason.clone(),
    })
}

fn admission_of(e: &Entry, me: &str) -> Option<AdmissionInfo> {
    let waiting = matches!(e.standing, Standing::Pending).then_some(&e.voters)?;
    Some(AdmissionInfo {
        votes: waiting.len(),
        need: ADMIT_NEED,
        voters: resolve_all(waiting, me),
        mine: waiting.iter().any(|v| v == ME),
        label: e.label.clone(),
    })
}

/// The own request of a computer that waits for a confirmation: no member has confirmed it yet.
fn own_admission(m: &DemoMembers) -> Option<AdmissionInfo> {
    m.me_pending.then(|| AdmissionInfo {
        votes: 0,
        need: ADMIT_NEED,
        voters: Vec::new(),
        mine: false,
        label: None,
    })
}

fn excluded_of(e: &Entry, me: &str) -> Option<ExcludedInfo> {
    match &e.standing {
        Standing::Excluded(x) => Some(ExcludedInfo {
            how: x.how,
            at_unix: AT_UNIX,
            voters: resolve_all(&x.voters, me),
        }),
        _ => None,
    }
}

fn row(m: &DemoMembers, e: &Entry, me: &str, me_short: &str) -> MemberRow {
    let is_self = e.id_full == ME;
    MemberRow {
        id: if is_self {
            me_short.to_string()
        } else {
            e.id.clone()
        },
        id_full: resolve(&e.id_full, me),
        name: e.name.clone(),
        is_self,
        online: e.online,
        admin: false,
        status: status_of(e),
        vote: vote_of(m, e, me),
        excluded: excluded_of(e, me),
        blocked_by_me: e.blocked,
        denied_by_me: denied_by_me(m, e),
        forget: forget_info(m, e, me_short),
        last_seen_sec: last_seen(e),
        last_refusal: None,
        admission: admission_of(e, me),
    }
}

/// Own row first, then members, unlisted, excluded; inside a group by name (nameless last), then id.
fn order_key(e: &Entry) -> (u8, u8, String, String) {
    let group = match e.standing {
        Standing::Member => 0,
        Standing::Pending => 1,
        Standing::Unlisted => 2,
        Standing::Excluded(_) => 3,
        Standing::Forgotten => 4,
    };
    let nameless = u8::from(e.name.is_none());
    (
        u8::from(e.id_full != ME) * (1 + group),
        nameless,
        e.name.clone().unwrap_or_default(),
        e.id_full.clone(),
    )
}

fn counts(rows: &[MemberRow]) -> Counts {
    let n = |status: &str| rows.iter().filter(|r| r.status == status).count();
    Counts {
        members: n("member"),
        voting: rows.iter().filter(|r| r.vote.is_some()).count(),
        excluded: n("excluded"),
        unlisted: n("unlisted"),
        pending: n("pending"),
        forgotten: n("forgotten"),
    }
}

pub(crate) fn build_reply(m: &DemoMembers, boot_id: &str, changed: Option<bool>) -> MembersReply {
    let me = demo_self_id_full(boot_id);
    let mut sorted: Vec<&Entry> = m.entries.iter().collect();
    sorted.sort_by_key(|e| order_key(e));
    let members: Vec<MemberRow> = sorted
        .into_iter()
        .map(|e| row(m, e, &me, boot_id))
        .collect();
    let (counts, me_admission) = (counts(&members), own_admission(m));
    MembersReply {
        me,
        me_excluded: m.me_excluded,
        me_removed: m.me_removed,
        me_pending: m.me_pending,
        me_admission,
        need: m.need(),
        kind: "Peer",
        admins: Vec::new(),
        me_admin: false,
        root: None,
        counts,
        members,
        changed,
    }
}

fn ensure_online(demo: &DemoNode, st: &DemoState) -> Result<(), MemberError> {
    if demo.no_network || st.status != NodeStatus::Online {
        return Err(MemberError::Action(ActionError::Busy(
            "the node is not in a network".to_string(),
        )));
    }
    Ok(())
}

impl DemoNode {
    pub(crate) fn members_list(&self) -> Result<MembersReply, MemberError> {
        let st = self.state.lock().unwrap();
        ensure_online(self, &st)?;
        Ok(build_reply(&st.members, &st.boot_id, None))
    }

    pub(crate) fn members_act(&self, act: &MemberActionReq) -> Result<MembersReply, MemberError> {
        let mut st = self.state.lock().unwrap();
        ensure_online(self, &st)?;
        let changed = st.members.apply(act)?;
        Ok(build_reply(&st.members, &st.boot_id, Some(changed)))
    }
}
