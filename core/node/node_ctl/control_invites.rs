//! Open invites for the page: the bodies of `NodeControl::open_invites` and `relabel_invite` for `NodeCtl`.
//! The decision who may ask and edit is made here; the records are read and rewritten by `invite::open`.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::invite::open::{OpenInvite, Relabel};
use crate::invite::store::Ledger;
use crate::invite::INVITE_TTL_SECS;
use crate::members::access::view::Verdicts;
use crate::members::membership::Members;
use crate::node::RunningNode;
use crate::transport::net::PeerId;
use crate::transport::net_mainline::unix_now;
use crate::web_gui::{ActionError, InviteRow, InvitesFuture, InvitesReply, MemberError};

use super::state::NodeCtl;

impl NodeCtl {
    pub(super) fn open_invites_impl(&self) -> InvitesFuture {
        match self.running_node_for_read() {
            Ok(node) => Box::pin(async move { serve(&node, None).await }),
            Err(e) => failed(MemberError::Action(e)),
        }
    }

    pub(super) fn relabel_invite_impl(
        &self,
        hash: String,
        for_whom: Option<String>,
    ) -> InvitesFuture {
        match self.running_node_for_action() {
            Ok(node) => Box::pin(async move { serve(&node, Some((hash, for_whom))).await }),
            Err(e) => failed(MemberError::Action(e)),
        }
    }
}

fn failed(e: MemberError) -> InvitesFuture {
    Box::pin(async move { Err(e) })
}

/// The label to set: the invite hash and the new note.
pub(super) type Edit = (String, Option<String>);

async fn serve(node: &RunningNode, edit: Option<Edit>) -> Result<InvitesReply, MemberError> {
    let store = Ledger::of(&node.domains);
    list_for(&store, &node.members, node.access.view.verdicts(), edit).await
}

fn refused(code: &'static str, message: &str) -> MemberError {
    MemberError::Refused {
        code,
        message: message.to_string(),
    }
}

fn internal(e: anyhow::Error) -> MemberError {
    MemberError::Action(ActionError::Internal(format!("{e:#}")))
}

/// The admins whose records count, or `not_admin` when the viewer is not one.
fn admins_of(v: Option<&Arc<Verdicts>>) -> Result<(PeerId, BTreeSet<PeerId>), MemberError> {
    let not_admin = || refused("not_admin", "only admins can do this in this network");
    let v = v.ok_or_else(not_admin)?;
    if !v.ledger.is_admin(&v.me) {
        return Err(not_admin());
    }
    Ok((v.me, v.ledger.admins.clone()))
}

/// The list for an admin, after the optional edit of a note.
pub(super) async fn list_for(
    store: &Ledger,
    members: &Members,
    verdicts: Option<Arc<Verdicts>>,
    edit: Option<Edit>,
) -> Result<InvitesReply, MemberError> {
    let (me, admins) = admins_of(verdicts.as_ref())?;
    let now = unix_now();
    if let Some((hash, note)) = edit {
        let done = store.relabel(&hash, note.as_deref(), now, Some(&admins));
        relabel_result(done.await.map_err(internal)?)?;
    }
    let open = store.open(now, Some(&admins)).await.map_err(internal)?;
    Ok(InvitesReply {
        invites: open.iter().map(|o| row_of(o, me, members)).collect(),
    })
}

fn relabel_result(r: Relabel) -> Result<(), MemberError> {
    match r {
        Relabel::Done => Ok(()),
        Relabel::NotFound => Err(MemberError::Action(ActionError::NotFound(
            "this invite has been used or has expired".to_string(),
        ))),
        Relabel::NotIssuer => Err(refused(
            "not_issuer",
            "only the admin who made this invite can rename it",
        )),
    }
}

fn row_of(o: &OpenInvite, me: PeerId, members: &Members) -> InviteRow {
    InviteRow {
        hash: o.hash.clone(),
        for_whom: o.for_whom.clone(),
        made_unix: o.made_unix,
        expires_unix: o.made_unix.saturating_add(INVITE_TTL_SECS),
        issuer: o.issuer.map(|p| p.to_string()),
        issuer_name: o.issuer.and_then(|p| members.name_of(p)),
        mine: o.issuer == Some(me),
    }
}

#[cfg(test)]
#[path = "control_invites_tests.rs"]
mod tests;
