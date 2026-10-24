//! Member actions of a managed network: who may do what, and the records of the role actions.
//!
//! What it does: `gate` answers the refusals that depend on the kind of the network and the role of this
//! node; `grant`, `revoke` and `close` write the records of power.
//! What it doesn't do: the checks shared by all networks and the votes of a peer network live in
//! `actions`.

use crate::transport::net::{NetResult, PeerId};
use crate::transport::net_mainline::unix_now;

use super::actions::{ActionRefusal, MemberAction};
use super::authority::Rules;
use super::record::{self, AclKind, AclValue, Via};
use super::runtime::Access;
use super::view::Verdicts;

type Refusal = Result<(), ActionRefusal>;

/// The refusals of the network kind, before the checks shared by all networks.
pub(super) fn gate(me: PeerId, rules: Rules, v: &Verdicts, act: &MemberAction) -> Refusal {
    match rules.root() {
        Some(root) => managed_gate(me, root, v, act),
        None => peer_gate(act),
    }
}

/// A network without administrators knows no role actions.
fn peer_gate(act: &MemberAction) -> Refusal {
    match act {
        MemberAction::Grant(_) | MemberAction::Revoke(_) | MemberAction::Close => {
            Err(ActionRefusal::NotManaged)
        }
        _ => Ok(()),
    }
}

fn managed_gate(me: PeerId, root: PeerId, v: &Verdicts, act: &MemberAction) -> Refusal {
    if v.ledger.closed.is_some() {
        return Err(ActionRefusal::Closed);
    }
    if matches!(act, MemberAction::Forget(_) | MemberAction::Unforget(_)) {
        return Err(ActionRefusal::PeerOnly);
    }
    if v.ledger.excluded.contains_key(&me) {
        return Ok(());
    }
    match act {
        MemberAction::Leave if is_last_admin(me, v) => Err(ActionRefusal::LastAdmin),
        MemberAction::Leave | MemberAction::Block(_) | MemberAction::Unblock(_) => Ok(()),
        other => power_gate(me, root, v, other),
    }
}

/// Whether the admission is the return of an excluded node: an administrator needs no request for it.
pub(super) fn returns(rules: Rules, v: &Verdicts, p: &PeerId) -> bool {
    rules.is_managed() && v.ledger.excluded.contains_key(p)
}

fn is_last_admin(me: PeerId, v: &Verdicts) -> bool {
    v.ledger.is_admin(&me) && v.ledger.admins.len() == 1
}

/// An action of power: only an administrator, and never against the creator.
fn power_gate(me: PeerId, root: PeerId, v: &Verdicts, act: &MemberAction) -> Refusal {
    if !v.ledger.is_admin(&me) {
        return Err(ActionRefusal::NotAdmin);
    }
    match act {
        MemberAction::Exclude(p, _) if *p == root => Err(ActionRefusal::NotAdmin),
        MemberAction::Grant(p) if !v.ledger.is_member(p) => Err(ActionRefusal::NotMember),
        MemberAction::Revoke(p) if *p == root => Err(ActionRefusal::NotAdmin),
        MemberAction::Revoke(p) if !v.ledger.is_member(p) => Err(ActionRefusal::NotMember),
        MemberAction::Revoke(p) if *p == me && is_last_admin(me, v) => {
            Err(ActionRefusal::LastAdmin)
        }
        _ => Ok(()),
    }
}

fn record_of(kind: AclKind, subject: PeerId) -> AclValue {
    AclValue {
        kind,
        subject,
        via: Via::None,
        made_unix: unix_now(),
        note: String::new(),
    }
}

/// Gives the role. The tally reads the time of the live record, so a role given again needs a new one:
/// the own takeback is withdrawn when there is one, otherwise the record is written afresh.
pub(super) async fn grant(a: &Access, v: &Verdicts, p: PeerId) -> NetResult<bool> {
    if v.ledger.is_admin(&p) {
        return Ok(false);
    }
    if record::withdraw(&*a.shared, AclKind::Revoke, p).await? {
        return Ok(true);
    }
    record::rewrite(&*a.shared, &record_of(AclKind::Grant, p))
        .await
        .map(|_| true)
}

/// Takes the role away; the record is stamped now, so it outdates every earlier grant.
pub(super) async fn revoke(a: &Access, v: &Verdicts, p: PeerId) -> NetResult<bool> {
    if !v.ledger.is_admin(&p) {
        return Ok(false);
    }
    record::rewrite(&*a.shared, &record_of(AclKind::Revoke, p))
        .await
        .map(|_| true)
}

/// Deletes the network: the record names its author as the subject.
pub(super) async fn close(a: &Access) -> NetResult<bool> {
    record::ensure(&*a.shared, &record_of(AclKind::Close, a.me)).await
}

#[cfg(test)]
#[path = "actions_managed_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "actions_managed_grant_tests.rs"]
mod grant_tests;

#[cfg(test)]
#[path = "actions_managed_support.rs"]
mod support;
