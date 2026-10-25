//! Member actions: exclude, unexclude, forget, unforget, admit, unadmit, block, unblock, leave.
//!
//! What it does: checks the action against the current verdicts, writes the own record (vote or
//! leave in the shared replica, block in the own one) and recomputes.
//! What it doesn't do: no parsing of ids or HTTP (the node control layer does).

use crate::node::output::say;
use crate::transport::net::{NetResult, PeerId};
use crate::transport::net_mainline::unix_now;

use super::actions_managed::{self, gate, returns};
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
    /// Make a member an administrator of a managed network.
    Grant(PeerId),
    /// Take the administrator role away.
    Revoke(PeerId),
    /// Delete the network.
    Close,
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
    /// The action needs an administrator and this node is not one.
    NotAdmin,
    /// The only administrator may neither leave nor give the role up.
    LastAdmin,
    /// The action exists only in a managed network.
    NotManaged,
    /// The action exists only in a network without administrators.
    PeerOnly,
    /// The network has been deleted.
    Closed,
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
    let after = v
        .ledger
        .last_admit_unix(&p)
        .map_or(0, |t| t.saturating_add(1));
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

/// Whether a forgotten node may still do the action: leave and the takebacks stay open.
fn keeps_when_forgotten(act: &MemberAction) -> bool {
    matches!(
        act,
        MemberAction::Leave
            | MemberAction::Unforget(_)
            | MemberAction::Unexclude(_)
            | MemberAction::Unblock(_)
    )
}

/// The checks in contract order: the rules of the network kind, left, self target, then the state of
/// the subject.
fn check(a: &Access, v: &Verdicts, act: &MemberAction) -> Result<(), ActionRefusal> {
    gate(a.me, a.rules, v, act)?;
    if matches!(act, MemberAction::Leave) {
        return Ok(());
    }
    refuse_if(has_left(a.me, v, act), ActionRefusal::Left)?;
    check_subject(a, v, act)
}

fn refuse_if(cond: bool, refusal: ActionRefusal) -> Result<(), ActionRefusal> {
    if cond {
        return Err(refusal);
    }
    Ok(())
}

/// The node is out of the list, or forgotten and the action is not one of the open ones.
fn has_left(me: PeerId, v: &Verdicts, act: &MemberAction) -> bool {
    let forgotten = v.ledger.is_forgotten(&me) && !keeps_when_forgotten(act);
    v.ledger.excluded.contains_key(&me) || forgotten
}

fn other_than(me: PeerId, p: &PeerId) -> Result<(), ActionRefusal> {
    refuse_if(*p == me, ActionRefusal::SelfTarget)
}

/// The self-target check and the state of the subject, by the kind of the action.
fn check_subject(a: &Access, v: &Verdicts, act: &MemberAction) -> Result<(), ActionRefusal> {
    use MemberAction::*;
    let me = a.me;
    match act {
        Exclude(p, _) => other_than(me, p).and_then(|_| exclude_known(v, p)),
        Admit(p) => other_than(me, p).and_then(|_| admit_pending(a, v, p)),
        Unadmit(p) => other_than(me, p).and_then(|_| unadmit_confirmed(me, v, p)),
        Forget(p) => other_than(me, p).and_then(|_| forget_allowed(a, v, p)),
        Unexclude(p) | Block(p) | Unblock(p) | Unforget(p) => other_than(me, p),
        Grant(_) | Revoke(_) | Close | Leave => Ok(()),
    }
}

fn exclude_known(v: &Verdicts, p: &PeerId) -> Result<(), ActionRefusal> {
    let known = v.ledger.admitted.contains(p) || v.ledger.excluded.contains_key(p);
    refuse_if(!known, ActionRefusal::NotMember)
}

fn admit_pending(a: &Access, v: &Verdicts, p: &PeerId) -> Result<(), ActionRefusal> {
    let waits = confirmed_by(a.me, v, p) == Some(false) || returns(a.rules, v, p);
    refuse_if(!waits, ActionRefusal::NotPending)
}

fn unadmit_confirmed(me: PeerId, v: &Verdicts, p: &PeerId) -> Result<(), ActionRefusal> {
    refuse_if(
        confirmed_by(me, v, p) != Some(true),
        ActionRefusal::NotConfirmed,
    )
}

fn forget_allowed(a: &Access, v: &Verdicts, p: &PeerId) -> Result<(), ActionRefusal> {
    refuse_if(!v.ledger.is_member(p), ActionRefusal::NotMember)?;
    refuse_if(a.members.ids().contains(p), ActionRefusal::Online)
}

/// Writes the record of the action; `true` — something changed.
async fn write(a: &Access, v: &Verdicts, act: &MemberAction) -> NetResult<bool> {
    match act {
        MemberAction::Grant(p) => actions_managed::grant(a, v, *p).await,
        MemberAction::Revoke(p) => actions_managed::revoke(a, v, *p).await,
        MemberAction::Close => actions_managed::close(a).await,
        _ => write_record(a, v, act).await,
    }
}

/// The votes, takebacks, blocks and leaving; the role actions are written by `write`.
async fn write_record(a: &Access, v: &Verdicts, act: &MemberAction) -> NetResult<bool> {
    use MemberAction::*;
    match act {
        Exclude(p, note) => exclude(a, v, p, note.as_deref()).await,
        Unexclude(p) => record::withdraw(&*a.shared, AclKind::Exclude, *p).await,
        Admit(p) => admit(a, p).await,
        Unadmit(p) => record::withdraw(&*a.shared, AclKind::Admit, *p).await,
        Block(p) => record::ensure(&*a.own, &value(AclKind::Block, *p)).await,
        Unblock(p) => record::withdraw(&*a.own, AclKind::Block, *p).await,
        Forget(p) => forget(a, v, *p).await,
        Unforget(p) => record::withdraw(&*a.shared, AclKind::Forget, *p).await,
        Leave => exclude(a, v, &a.me, None).await,
        Grant(_) | Revoke(_) | Close => Ok(false),
    }
}

/// An exclusion vote (the own leave is the vote against the own id); nothing to do when it stands.
async fn exclude(a: &Access, v: &Verdicts, p: &PeerId, note: Option<&str>) -> NetResult<bool> {
    if v.ledger.excluded.contains_key(p) {
        return Ok(false);
    }
    let mut vote = noted(AclKind::Exclude, *p, note.unwrap_or_default().to_owned());
    if a.rules.is_managed() && *p != a.me {
        return record::rewrite(&*a.shared, &vote).await.map(|_| true);
    }
    if !a.rules.is_managed() {
        vote.via = record::VOTE_AT_MOMENT;
    }
    record::ensure(&*a.shared, &vote).await
}

async fn admit(a: &Access, p: &PeerId) -> NetResult<bool> {
    let vote = value(AclKind::Admit, *p);
    if a.rules.is_managed() {
        return record::rewrite(&*a.shared, &vote).await.map(|_| true);
    }
    record::ensure(&*a.shared, &vote).await
}

async fn forget(a: &Access, v: &Verdicts, p: PeerId) -> NetResult<bool> {
    if voted_to_forget(a.me, v, &p) {
        return Ok(false);
    }
    record::rewrite(&*a.shared, &forget_vote(v, p))
        .await
        .map(|_| true)
}

/// Applies the action and recomputes the verdicts. `Ok(true)` — the lists changed.
pub async fn apply(a: &Access, act: MemberAction) -> Result<bool, ActionRefusal> {
    let verdicts = a.refresh().await.map_err(write_refusal)?;
    check(a, &verdicts, &act)?;
    let changed = write(a, &verdicts, &act).await.map_err(write_refusal)?;
    let changed = unchanged_refusal(&act, changed)?;
    refresh_after(a, changed).await;
    Ok(changed)
}

fn write_refusal(e: impl std::fmt::Display) -> ActionRefusal {
    ActionRefusal::Write(e.to_string())
}

/// An `Unforget` that changed nothing had no own vote to withdraw.
fn unchanged_refusal(act: &MemberAction, changed: bool) -> Result<bool, ActionRefusal> {
    if !changed && matches!(act, MemberAction::Unforget(_)) {
        return Err(ActionRefusal::NotForgotten);
    }
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

#[cfg(test)]
#[path = "actions_forgotten_me_tests.rs"]
mod forgotten_me_tests;

#[cfg(test)]
#[path = "actions_overflow_tests.rs"]
mod overflow_tests;
