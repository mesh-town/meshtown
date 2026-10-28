//! `GET /api/members` of the demo node: the list as the page sees it, and the `NodeControl` entry points.

use meshtown_core::web_gui::{
    AdmissionInfo, Counts, ExcludedInfo, MemberActionReq, MemberError, MemberRow, MembersReply,
    NetRoles, NodeStatus,
};

use meshtown_core::web_gui::ActionError;

use crate::managed::own_id_as_me;
use crate::members::{DemoMembers, Entry, Standing, ME};
use crate::node::{DemoNode, DemoState};

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
    }
}

fn denied_by_me(m: &DemoMembers, e: &Entry) -> Option<&'static str> {
    match (&e.standing, e.id_full == ME) {
        (_, true) => None,
        _ if m.me_excluded => Some("left"),
        (Standing::Excluded(_), _) => Some("excluded"),
        _ if e.blocked => Some("blocked"),
        (Standing::Unlisted | Standing::Pending, _) => Some("unlisted"),
        _ => None,
    }
}

fn admission_of(e: &Entry) -> Option<AdmissionInfo> {
    matches!(e.standing, Standing::Pending).then(|| AdmissionInfo {
        label: e.label.clone(),
    })
}

/// Seconds since the last sign of life: zero while online, unknown otherwise.
fn last_seen(e: &Entry) -> Option<u64> {
    e.online.then_some(0)
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
        admin: m.is_admin(&e.id_full),
        status: status_of(e),
        excluded: excluded_of(e, me),
        blocked_by_me: e.blocked,
        denied_by_me: denied_by_me(m, e),
        last_seen_sec: last_seen(e),
        last_refusal: None,
        admission: admission_of(e),
    }
}

/// Own row first, then members, unlisted, excluded; inside a group by name (nameless last), then id.
fn order_key(e: &Entry) -> (u8, u8, String, String) {
    let group = match e.standing {
        Standing::Member => 0,
        Standing::Pending => 1,
        Standing::Unlisted => 2,
        Standing::Excluded(_) => 3,
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
        excluded: n("excluded"),
        unlisted: n("unlisted"),
        pending: n("pending"),
    }
}

/// The administrators as full ids, ascending.
fn admins_of(m: &DemoMembers, me: &str) -> Vec<String> {
    let mut ids = resolve_all(&m.roles.admins, me);
    ids.sort();
    ids
}

pub(crate) fn build_reply(m: &DemoMembers, boot_id: &str, changed: Option<bool>) -> MembersReply {
    let me = demo_self_id_full(boot_id);
    let mut sorted: Vec<&Entry> = m.entries.iter().collect();
    sorted.sort_by_key(|e| order_key(e));
    let members: Vec<MemberRow> = sorted
        .into_iter()
        .filter(|_| !m.is_closed())
        .map(|e| row(m, e, &me, boot_id))
        .collect();
    let counts = counts(&members);
    MembersReply {
        me: me.clone(),
        me_excluded: m.me_excluded,
        me_pending: false,
        me_admission: None,
        admins: admins_of(m, &me),
        me_admin: m.is_admin(ME),
        root: Some(resolve(&m.roles.root, &me)),
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

    /// The roles of the demo network.
    pub(crate) fn roles_of_node(&self) -> Option<NetRoles> {
        let st = self.state.lock().unwrap();
        Some(NetRoles::from_reply(&build_reply(
            &st.members,
            &st.boot_id,
            None,
        )))
    }

    pub(crate) fn members_act(&self, act: &MemberActionReq) -> Result<MembersReply, MemberError> {
        let mut st = self.state.lock().unwrap();
        ensure_online(self, &st)?;
        let act = own_id_as_me(act.clone(), &st.boot_id);
        let changed = st.members.apply(&act)?;
        Ok(build_reply(&st.members, &st.boot_id, Some(changed)))
    }
}

#[cfg(test)]
#[path = "members_reply_tests.rs"]
mod tests;
