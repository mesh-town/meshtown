//! Page actions on the membership list: the bodies of `NodeControl::members` and `member_action` for `NodeCtl`.
//! The request is parsed here, the decision is made by `access::actions::apply`.

use std::sync::Arc;

use crate::members::access::actions::{apply, ActionRefusal, MemberAction};
use crate::members::access::report::{members_reply, stamp_last_seen};
use crate::members::access::view::Verdicts;
use crate::node::RunningNode;
use crate::transport::net::PeerId;
use crate::transport::net_mainline::unix_now;
use crate::web_gui::{ActionError, MemberActionReq, MemberError, MemberFuture, MembersReply};

use super::members_seen::seen_ages;
use super::state::NodeCtl;

impl NodeCtl {
    pub(super) fn members_impl(&self) -> MemberFuture {
        match self.running_node_for_action() {
            Ok(node) => Box::pin(async move { reply_of(&node, None).await }),
            Err(e) => failed(MemberError::Action(e)),
        }
    }

    pub(super) fn member_action_impl(&self, req: MemberActionReq) -> MemberFuture {
        let node = match self.running_node_for_action() {
            Ok(node) => node,
            Err(e) => return failed(MemberError::Action(e)),
        };
        match parse_action(req) {
            Ok(act) => Box::pin(async move { run_action(&node, act).await }),
            Err(e) => failed(e),
        }
    }
}

fn failed(e: MemberError) -> MemberFuture {
    Box::pin(async move { Err(e) })
}

fn invalid(what: &str) -> MemberError {
    MemberError::Action(ActionError::Invalid(format!(
        "{what}: a full node id (64 hex digits) is needed"
    )))
}

fn parse_id(id: &str) -> Result<PeerId, MemberError> {
    id.trim().parse().map_err(|_| invalid("id"))
}

/// The page request as a list action; a bad id is `Invalid`.
pub(super) fn parse_action(req: MemberActionReq) -> Result<MemberAction, MemberError> {
    match req {
        MemberActionReq::Exclude { id, reason } => {
            Ok(MemberAction::Exclude(parse_id(&id)?, reason))
        }
        MemberActionReq::Unexclude(id) => Ok(MemberAction::Unexclude(parse_id(&id)?)),
        MemberActionReq::Admit(id) => Ok(MemberAction::Admit(parse_id(&id)?)),
        MemberActionReq::Unadmit(id) => Ok(MemberAction::Unadmit(parse_id(&id)?)),
        MemberActionReq::Block(id) => Ok(MemberAction::Block(parse_id(&id)?)),
        MemberActionReq::Unblock(id) => Ok(MemberAction::Unblock(parse_id(&id)?)),
        MemberActionReq::Forget(id) => Ok(MemberAction::Forget(parse_id(&id)?)),
        MemberActionReq::Unforget(id) => Ok(MemberAction::Unforget(parse_id(&id)?)),
        MemberActionReq::Leave => Ok(MemberAction::Leave),
    }
}

fn refused(code: &'static str, message: &str) -> MemberError {
    MemberError::Refused {
        code,
        message: message.to_string(),
    }
}

/// A refusal of the action as the page error: a code with a message, or an internal failure.
pub(super) fn refusal_error(r: ActionRefusal) -> MemberError {
    match r {
        ActionRefusal::SelfTarget => refused("self_target", "this is your own node"),
        ActionRefusal::NotMember => refused("not_member", "the node is not in the list"),
        ActionRefusal::Left => refused("left", "this node has left the network"),
        ActionRefusal::NotPending => {
            refused("not_pending", "This node isn't waiting for confirmation")
        }
        ActionRefusal::NotConfirmed => refused(
            "not_confirmed",
            "You haven't confirmed this node, or it has already joined",
        ),
        ActionRefusal::Online => refused("online", "The node is connected right now"),
        ActionRefusal::NotForgotten => {
            refused("not_forgotten", "You haven't proposed removing this node")
        }
        ActionRefusal::Write(m) => MemberError::Action(ActionError::Internal(m)),
    }
}

async fn run_action(node: &RunningNode, act: MemberAction) -> Result<MembersReply, MemberError> {
    let changed = apply(&node.access, act).await.map_err(refusal_error)?;
    reply_of(node, Some(changed)).await
}

async fn reply_of(node: &RunningNode, changed: Option<bool>) -> Result<MembersReply, MemberError> {
    let v = verdicts_of(node).await?;
    let mut reply = members_reply(&v, &node.members, &node.access.view.refusals(), changed);
    stamp_last_seen(&mut reply, &seen_ages(node, unix_now()).await);
    Ok(reply)
}

/// The published verdicts, or a fresh recompute when nothing is published yet.
async fn verdicts_of(node: &RunningNode) -> Result<Arc<Verdicts>, MemberError> {
    match node.access.view.verdicts() {
        Some(v) => Ok(v),
        None => node
            .access
            .refresh()
            .await
            .map_err(|e| MemberError::Action(ActionError::Internal(e.to_string()))),
    }
}

#[cfg(test)]
#[path = "control_members_tests.rs"]
mod tests;
