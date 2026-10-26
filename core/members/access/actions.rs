//! Member actions: exclude, unexclude, admit, block, unblock, grant, revoke, close, leave.
//!
//! What it does: checks the action against the current verdicts, writes the own record (exclusion,
//! admission or leave in the shared replica, block in the own one) and recomputes.
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
    /// The node and an optional note with the reason (stored in the exclusion record).
    Exclude(PeerId, Option<String>),
    Unexclude(PeerId),
    /// Let in a node that waits to join.
    Admit(PeerId),
    Block(PeerId),
    Unblock(PeerId),
    /// Make a member an administrator.
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
    /// The action needs an administrator and this node is not one.
    NotAdmin,
    /// The only administrator may neither leave nor give the role up.
    LastAdmin,
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

/// The checks in contract order: the rules of the network, left, self target, then the state of the subject.
fn check(a: &Access, v: &Verdicts, act: &MemberAction) -> Result<(), ActionRefusal> {
    gate(a.me, a.root, v, act)?;
    if matches!(act, MemberAction::Leave) {
        return Ok(());
    }
    refuse_if(has_left(a.me, v), ActionRefusal::Left)?;
    check_subject(a, v, act)
}

fn refuse_if(cond: bool, refusal: ActionRefusal) -> Result<(), ActionRefusal> {
    if cond {
        return Err(refusal);
    }
    Ok(())
}

/// The node is out of the list.
fn has_left(me: PeerId, v: &Verdicts) -> bool {
    v.ledger.excluded.contains_key(&me)
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
        Admit(p) => other_than(me, p).and_then(|_| admit_pending(v, p)),
        Unexclude(p) | Block(p) | Unblock(p) => other_than(me, p),
        Grant(_) | Revoke(_) | Close | Leave => Ok(()),
    }
}

fn exclude_known(v: &Verdicts, p: &PeerId) -> Result<(), ActionRefusal> {
    let known = v.ledger.admitted.contains(p) || v.ledger.excluded.contains_key(p);
    refuse_if(!known, ActionRefusal::NotMember)
}

fn admit_pending(v: &Verdicts, p: &PeerId) -> Result<(), ActionRefusal> {
    let waits = v.ledger.admissions.contains_key(p) || returns(v, p);
    refuse_if(!waits, ActionRefusal::NotPending)
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

/// The exclusions, admissions, takebacks, blocks and leaving; the role actions are written by `write`.
async fn write_record(a: &Access, v: &Verdicts, act: &MemberAction) -> NetResult<bool> {
    use MemberAction::*;
    match act {
        Exclude(p, note) => exclude(a, v, p, note.as_deref()).await,
        Unexclude(p) => record::withdraw(&*a.shared, AclKind::Exclude, *p).await,
        Admit(p) => admit(a, p).await,
        Block(p) => record::ensure(&*a.own, &value(AclKind::Block, *p)).await,
        Unblock(p) => record::withdraw(&*a.own, AclKind::Block, *p).await,
        Leave => exclude(a, v, &a.me, None).await,
        Grant(_) | Revoke(_) | Close => Ok(false),
    }
}

/// An exclusion (the own leave is the exclusion of the own id); nothing to do when it stands.
async fn exclude(a: &Access, v: &Verdicts, p: &PeerId, note: Option<&str>) -> NetResult<bool> {
    if v.ledger.excluded.contains_key(p) {
        return Ok(false);
    }
    let record = noted(AclKind::Exclude, *p, note.unwrap_or_default().to_owned());
    if *p != a.me {
        return record::rewrite(&*a.shared, &record).await.map(|_| true);
    }
    record::ensure(&*a.shared, &record).await
}

async fn admit(a: &Access, p: &PeerId) -> NetResult<bool> {
    let record = value(AclKind::Admit, *p);
    record::rewrite(&*a.shared, &record).await.map(|_| true)
}

/// Applies the action and recomputes the verdicts. `Ok(true)` — the lists changed.
pub async fn apply(a: &Access, act: MemberAction) -> Result<bool, ActionRefusal> {
    let verdicts = a.refresh().await.map_err(write_refusal)?;
    check(a, &verdicts, &act)?;
    let changed = write(a, &verdicts, &act).await.map_err(write_refusal)?;
    refresh_after(a, changed).await;
    Ok(changed)
}

fn write_refusal(e: impl std::fmt::Display) -> ActionRefusal {
    ActionRefusal::Write(e.to_string())
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
