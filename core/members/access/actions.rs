//! Member actions: exclude, unexclude, forget, unforget, admit, unadmit, block, unblock, leave.
//!
//! What it does: checks the action against the current verdicts, writes the own record (vote or
//! leave in the shared replica, block in the own one) and recomputes.
//! What it doesn't do: no parsing of ids or HTTP (the node control layer does).

use std::collections::HashSet;

use crate::node::output::say;
use crate::transport::net::{NetResult, PeerId};
use crate::transport::net_mainline::unix_now;

use super::record::{self, AclKind, AclValue, Via};
use super::runtime::Access;
use super::view::Verdicts;

/// What the user asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberAction {
    /// The node and an optional note with the reason (stored in the vote record).
    Exclude(PeerId, Option<String>),
    Unexclude(PeerId),
    /// Confirm a node that waits to join (a vote for its admission).
    Admit(PeerId),
    /// Take the own confirmation back while the node has not joined yet.
    Unadmit(PeerId),
    Block(PeerId),
    Unblock(PeerId),
    /// Vote to remove a node that is not connected from the member list.
    Forget(PeerId),
    /// Take the own removal vote back.
    Unforget(PeerId),
    Leave,
}

/// Why an action is not done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionRefusal {
    SelfTarget,
    NotMember,
    Left,
    NotPending,
    NotConfirmed,
    /// The node to remove is connected to this one right now.
    Online,
    /// The own removal vote of the node does not exist.
    NotForgotten,
    Write(String),
}

fn value(kind: AclKind, subject: PeerId) -> AclValue {
    noted(kind, subject, String::new())
}

fn noted(kind: AclKind, subject: PeerId, note: String) -> AclValue {
    stamped(kind, subject, note, unix_now())
}

fn stamped(kind: AclKind, subject: PeerId, note: String, made_unix: u64) -> AclValue {
    AclValue {
        kind,
        subject,
        via: Via::None,
        made_unix,
        note,
    }
}

/// The removal vote: stamped after every admission of the subject, so that the vote counts.
fn forget_vote(v: &Verdicts, p: PeerId) -> AclValue {
    let after = v.ledger.last_admit_unix(&p).map_or(0, |t| t + 1);
    stamped(AclKind::Forget, p, String::new(), unix_now().max(after))
}

/// Whether the vote of `me` is already in the current circle of `p`.
fn voted_to_forget(me: PeerId, v: &Verdicts, p: &PeerId) -> bool {
    let circle = v.ledger.forgets.get(p);
    circle.is_some_and(|round| round.voters.contains(&me))
}

/// Whether the request of `p` has the vote of `me` counted.
fn confirmed_by(me: PeerId, v: &Verdicts, p: &PeerId) -> Option<bool> {
    v.ledger.admissions.get(p).map(|a| a.voters.contains(&me))
}

/// The checks in contract order: left, self target, then the state of the subject.
fn check(
    me: PeerId,
    v: &Verdicts,
    online: &HashSet<PeerId>,
    act: &MemberAction,
) -> Result<(), ActionRefusal> {
    let left = v.ledger.excluded.contains_key(&me);
    match (act.clone(), left) {
        (MemberAction::Leave, _) => Ok(()),
        (_, true) => Err(ActionRefusal::Left),
        (
            MemberAction::Exclude(p, _)
            | MemberAction::Unexclude(p)
            | MemberAction::Admit(p)
            | MemberAction::Unadmit(p)
            | MemberAction::Block(p)
            | MemberAction::Unblock(p)
            | MemberAction::Forget(p)
            | MemberAction::Unforget(p),
            _,
        ) if p == me => Err(ActionRefusal::SelfTarget),
        (MemberAction::Exclude(p, _), _)
            if !v.ledger.admitted.contains(&p) && !v.ledger.excluded.contains_key(&p) =>
        {
            Err(ActionRefusal::NotMember)
        }
        (MemberAction::Forget(p), _) if !v.ledger.is_member(&p) => Err(ActionRefusal::NotMember),
        (MemberAction::Forget(p), _) if online.contains(&p) => Err(ActionRefusal::Online),
        (MemberAction::Admit(p), _) if confirmed_by(me, v, &p) != Some(false) => {
            Err(ActionRefusal::NotPending)
        }
        (MemberAction::Unadmit(p), _) if confirmed_by(me, v, &p) != Some(true) => {
            Err(ActionRefusal::NotConfirmed)
        }
        _ => Ok(()),
    }
}

/// Writes the record of the action; `true` — something changed.
async fn write(a: &Access, v: &Verdicts, act: MemberAction) -> NetResult<bool> {
    let shared = &*a.shared;
    let own = &*a.own;
    match act {
        MemberAction::Exclude(p, _) if v.ledger.excluded.contains_key(&p) => Ok(false),
        MemberAction::Exclude(p, note) => {
            record::ensure(
                shared,
                &noted(AclKind::Exclude, p, note.unwrap_or_default()),
            )
            .await
        }
        MemberAction::Unexclude(p) => record::withdraw(shared, AclKind::Exclude, p).await,
        MemberAction::Admit(p) => record::ensure(shared, &value(AclKind::Admit, p)).await,
        MemberAction::Unadmit(p) => record::withdraw(shared, AclKind::Admit, p).await,
        MemberAction::Block(p) => record::ensure(own, &value(AclKind::Block, p)).await,
        MemberAction::Unblock(p) => record::withdraw(own, AclKind::Block, p).await,
        MemberAction::Forget(p) if voted_to_forget(a.me, v, &p) => Ok(false),
        MemberAction::Forget(p) => record::rewrite(shared, &forget_vote(v, p))
            .await
            .map(|_| true),
        MemberAction::Unforget(p) => record::withdraw(shared, AclKind::Forget, p).await,
        MemberAction::Leave if v.ledger.excluded.contains_key(&a.me) => Ok(false),
        MemberAction::Leave => record::ensure(shared, &value(AclKind::Exclude, a.me)).await,
    }
}

/// Applies the action and recomputes the verdicts. `Ok(true)` — the lists changed.
pub async fn apply(a: &Access, act: MemberAction) -> Result<bool, ActionRefusal> {
    let verdicts = a
        .refresh()
        .await
        .map_err(|e| ActionRefusal::Write(e.to_string()))?;
    check(a.me, &verdicts, &a.members.ids(), &act)?;
    let unforget = matches!(act, MemberAction::Unforget(_));
    let changed = write(a, &verdicts, act)
        .await
        .map_err(|e| ActionRefusal::Write(e.to_string()))?;
    if unforget && !changed {
        return Err(ActionRefusal::NotForgotten);
    }
    refresh_after(a, changed).await;
    Ok(changed)
}

async fn refresh_after(a: &Access, changed: bool) {
    if !changed {
        return;
    }
    if let Err(e) = a.refresh().await {
        say(
            "access",
            format!("acl recompute after an action failed: {e}"),
        );
    }
}

#[cfg(test)]
#[path = "actions_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "actions_forget_tests.rs"]
mod forget_tests;
