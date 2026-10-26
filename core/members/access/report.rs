//! The member list for the web page: rows, statuses, exclusions, refusals.
//!
//! What it does: turns the verdicts, the connection state and the last refusals into the reply of
//! `GET /api/members`.
//! What it doesn't do: no replica reads and no decisions; every verdict comes from `Verdicts::check`.

use std::collections::{BTreeMap, BTreeSet};

use crate::members::membership::Members;
use crate::node::output::short_peer_id;
use crate::transport::net::PeerId;
use crate::web_gui::{AdmissionInfo, Counts, ExcludedInfo, MemberRow, MembersReply, RefusalInfo};

use super::abilities::admins_of;
use super::tally::How;
use super::view::{Deny, Refusal, Side, Verdicts};

/// Everything the page knows about; the own node is always among them.
fn all_ids(v: &Verdicts, members: &Members, refusals: &[(PeerId, Refusal)]) -> BTreeSet<PeerId> {
    let mut ids: BTreeSet<PeerId> = v.ledger.admitted.clone();
    ids.extend(v.ledger.excluded.keys().copied());
    ids.extend(v.ledger.admissions.keys().copied());
    ids.extend(v.blocks.iter().copied());
    ids.extend(members.names().into_keys());
    ids.extend(members.ids());
    ids.extend(refusals.iter().map(|(p, _)| *p));
    ids.insert(v.me);
    ids
}

fn status_of(v: &Verdicts, p: &PeerId) -> &'static str {
    let waiting = v.ledger.admissions.contains_key(p);
    match (
        v.ledger.excluded.contains_key(p),
        v.ledger.admitted.contains(p),
        waiting,
    ) {
        (true, ..) => "excluded",
        (false, true, _) => "member",
        (false, false, true) => "pending",
        (false, false, false) => "unlisted",
    }
}

fn full(ids: &[PeerId]) -> Vec<String> {
    ids.iter().map(|p| p.to_string()).collect()
}

fn admission_of(v: &Verdicts, p: &PeerId) -> Option<AdmissionInfo> {
    let a = v.ledger.admissions.get(p)?;
    Some(AdmissionInfo {
        label: a.label.clone(),
    })
}

/// True when another node turned this one away as not admitted.
fn refused_as_unlisted(refusals: &[(PeerId, Refusal)]) -> bool {
    refusals
        .iter()
        .any(|(_, r)| r.side == Side::Them && r.deny == Deny::Unlisted)
}

/// This node's own request, shown while it waits and others have turned it away.
fn me_admission_of(v: &Verdicts, refusals: &[(PeerId, Refusal)]) -> Option<AdmissionInfo> {
    admission_of(v, &v.me).filter(|_| refused_as_unlisted(refusals))
}

fn excluded_of(v: &Verdicts, p: &PeerId) -> Option<ExcludedInfo> {
    let e = v.ledger.excluded.get(p)?;
    let how = match e.how {
        How::Votes => "removed",
        How::Left => "left",
    };
    Some(ExcludedInfo {
        how,
        at_unix: e.at_micros / 1_000_000,
        voters: full(&e.voters),
    })
}

fn refusal_of(refusals: &[(PeerId, Refusal)], p: &PeerId) -> Option<RefusalInfo> {
    let (_, r) = refusals.iter().find(|(id, _)| id == p)?;
    let side = match r.side {
        Side::Me => "me",
        Side::Them => "them",
    };
    Some(RefusalInfo {
        side,
        reason: r.deny.code(),
        at_unix: r.at_unix,
    })
}

fn row(
    v: &Verdicts,
    members: &Members,
    refusals: &[(PeerId, Refusal)],
    online: &BTreeSet<PeerId>,
    p: PeerId,
) -> MemberRow {
    MemberRow {
        id: short_peer_id(&p),
        id_full: p.to_string(),
        name: members.name_of(p),
        is_self: p == v.me,
        online: p == v.me || online.contains(&p),
        admin: v.ledger.is_admin(&p),
        status: status_of(v, &p),
        excluded: excluded_of(v, &p),
        blocked_by_me: v.blocks.contains(&p),
        denied_by_me: v.check(p).err().map(|d| d.code()),
        last_seen_sec: None,
        last_refusal: refusal_of(refusals, &p),
        admission: admission_of(v, &p).filter(|_| p != v.me),
    }
}

/// Own row first, then members, pending, unlisted, excluded; by name (nameless last), then by id.
fn order_key(r: &MemberRow) -> (u8, u8, bool, String, String) {
    let rank = match (r.is_self, r.status) {
        (true, _) => 0,
        (_, "member") => 1,
        (_, "pending") => 2,
        (_, "unlisted") => 3,
        _ => 4,
    };
    (
        rank,
        0,
        r.name.is_none(),
        r.name.clone().unwrap_or_default(),
        r.id_full.clone(),
    )
}

fn count_status(rows: &[MemberRow], status: &str) -> usize {
    rows.iter().filter(|r| r.status == status).count()
}

fn counts_of(rows: &[MemberRow]) -> Counts {
    Counts {
        members: count_status(rows, "member"),
        excluded: count_status(rows, "excluded"),
        unlisted: count_status(rows, "unlisted"),
        pending: count_status(rows, "pending"),
    }
}

/// The reply of `GET /api/members`; `changed` is set for replies to actions.
pub fn members_reply(
    v: &Verdicts,
    members: &Members,
    refusals: &[(PeerId, Refusal)],
    changed: Option<bool>,
) -> MembersReply {
    let online: BTreeSet<PeerId> = members.ids().into_iter().collect();
    let mut rows: Vec<MemberRow> = all_ids(v, members, refusals)
        .into_iter()
        .map(|p| row(v, members, refusals, &online, p))
        .collect();
    rows.sort_by_key(order_key);
    let me_admission = me_admission_of(v, refusals);
    MembersReply {
        me: v.me.to_string(),
        me_excluded: v.ledger.excluded.contains_key(&v.me),
        me_pending: me_admission.is_some(),
        me_admission,
        admins: admins_of(v),
        me_admin: v.ledger.is_admin(&v.me),
        root: None,
        counts: counts_of(&rows),
        members: rows,
        changed,
    }
}

/// What the state snapshot says about the list: how many requests wait for my confirmation and whether I am out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoteSummary {
    pub me_excluded: bool,
    /// Requests of other nodes that wait for my confirmation.
    pub admitting: u64,
    pub me_pending: bool,
}

/// True for a waiting request this node has not confirmed yet.
fn awaits_my_confirmation(r: &MemberRow) -> bool {
    r.status == "pending" && !r.is_self
}

/// Counts the rows that wait for this node's confirmation.
pub fn vote_summary(r: &MembersReply) -> VoteSummary {
    let admitting = r
        .members
        .iter()
        .filter(|row| awaits_my_confirmation(row))
        .count() as u64;
    VoteSummary {
        me_excluded: r.me_excluded,
        admitting,
        me_pending: r.me_pending,
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "report_summary_tests.rs"]
mod summary_tests;

#[cfg(test)]
#[path = "report_admission_tests.rs"]
mod admission_tests;

/// Sets `last_seen_sec` of each row from the map of ages (seconds since the last sign of life) by id.
pub fn stamp_last_seen(reply: &mut MembersReply, seen: &BTreeMap<PeerId, u64>) {
    for row in &mut reply.members {
        row.last_seen_sec = row
            .id_full
            .parse::<PeerId>()
            .ok()
            .and_then(|p| seen.get(&p).copied());
    }
}
