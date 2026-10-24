//! Roles of a managed demo network: the administrators, the creator (root) and the deleted state, plus the
//! refusals the real node gives for the role actions. A peer network has no `Roles` at all.

use meshtown_core::web_gui::{MemberActionReq, MemberError};

use crate::members::{refused, DemoMembers, Standing, ME};
use crate::members_reply::demo_self_id_full;

/// What the demo plays: a peer network, a managed one that I created, or a managed one I only joined.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ManagedMode {
    #[default]
    Peer,
    /// I am the creator and an administrator; one more member is an administrator.
    Admin,
    /// I am an ordinary member; two other members are administrators, the first one is the creator.
    Member,
}

/// Ids are the entry ids of the list, `ME` for this node.
pub(crate) struct Roles {
    pub(crate) root: String,
    pub(crate) admins: Vec<String>,
    pub(crate) closed: bool,
}

impl Roles {
    /// `others` are the ids of the two listed peers, in list order.
    pub(crate) fn new(mode: ManagedMode, others: &[String]) -> Option<Roles> {
        let [one, two] = others else { return None };
        let (root, admins) = match mode {
            ManagedMode::Peer => return None,
            ManagedMode::Admin => (ME, vec![ME, one]),
            ManagedMode::Member => (one.as_str(), vec![one.as_str(), two]),
        };
        Some(Roles {
            root: root.to_string(),
            admins: admins.into_iter().map(str::to_string).collect(),
            closed: false,
        })
    }
}

/// The own id of this node, as the page sends it, becomes `ME`, as the list names it.
pub(crate) fn own_id_as_me(act: MemberActionReq, boot_id: &str) -> MemberActionReq {
    let me = demo_self_id_full(boot_id);
    match act {
        MemberActionReq::Grant(id) if id == me => MemberActionReq::Grant(ME.to_string()),
        MemberActionReq::Revoke(id) if id == me => MemberActionReq::Revoke(ME.to_string()),
        other => other,
    }
}

fn is_role_act(act: &MemberActionReq) -> bool {
    matches!(
        act,
        MemberActionReq::Grant(_) | MemberActionReq::Revoke(_) | MemberActionReq::Close
    )
}

impl DemoMembers {
    pub(crate) fn is_admin(&self, id: &str) -> bool {
        self.roles
            .as_ref()
            .is_some_and(|r| r.admins.iter().any(|a| a == id))
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.roles.as_ref().is_some_and(|r| r.closed)
    }

    fn listed(&self, id: &str) -> bool {
        match id == ME {
            true => !self.me_excluded,
            false => self.entries.iter().any(|e| {
                e.id_full == id.to_ascii_lowercase() && matches!(e.standing, Standing::Member)
            }),
        }
    }

    /// The refusals that depend on the kind of the network and on my role, before everything else.
    pub(crate) fn gate(&self, act: &MemberActionReq) -> Result<(), MemberError> {
        let Some(roles) = &self.roles else {
            return peer_gate(act);
        };
        if roles.closed {
            return Err(refused("network_deleted", "this network has been deleted"));
        }
        if matches!(
            act,
            MemberActionReq::Forget(_) | MemberActionReq::Unforget(_)
        ) {
            return Err(refused("peer_only", "only a peer network forgets members"));
        }
        if self.me_excluded {
            return Ok(());
        }
        match act {
            MemberActionReq::Leave if self.is_last_admin(ME) => Err(last_admin()),
            MemberActionReq::Leave | MemberActionReq::Block(_) | MemberActionReq::Unblock(_) => {
                Ok(())
            }
            other => self.power_gate(roles, other),
        }
    }

    fn is_last_admin(&self, id: &str) -> bool {
        self.is_admin(id) && self.roles.as_ref().is_some_and(|r| r.admins.len() == 1)
    }

    /// Actions of power: administrators only, and never against the creator.
    fn power_gate(&self, roles: &Roles, act: &MemberActionReq) -> Result<(), MemberError> {
        if !self.is_admin(ME) {
            return Err(refused("not_admin", "only administrators can do this"));
        }
        match act {
            MemberActionReq::Exclude { id, .. } | MemberActionReq::Revoke(id)
                if *id == roles.root =>
            {
                Err(refused(
                    "not_admin",
                    "the creator of the network keeps the role",
                ))
            }
            MemberActionReq::Grant(id) | MemberActionReq::Revoke(id) if !self.listed(id) => {
                Err(refused("not_member", "the node is not in the member list"))
            }
            MemberActionReq::Revoke(id) if id == ME && self.is_last_admin(ME) => Err(last_admin()),
            _ => Ok(()),
        }
    }

    /// Grant, revoke and delete; `None` for the actions that are not about roles.
    pub(crate) fn role_act(&mut self, act: &MemberActionReq) -> Option<bool> {
        let roles = self.roles.as_mut()?;
        match act {
            MemberActionReq::Grant(id) if !roles.admins.contains(id) => {
                roles.admins.push(id.clone());
                Some(true)
            }
            MemberActionReq::Revoke(id) if roles.admins.contains(id) => {
                roles.admins.retain(|a| a != id);
                Some(true)
            }
            MemberActionReq::Close => {
                roles.closed = true;
                Some(true)
            }
            MemberActionReq::Grant(_) | MemberActionReq::Revoke(_) => Some(false),
            _ => None,
        }
    }
}

fn peer_gate(act: &MemberActionReq) -> Result<(), MemberError> {
    match is_role_act(act) {
        true => Err(refused("not_managed", "this is a peer network")),
        false => Ok(()),
    }
}

fn last_admin() -> MemberError {
    refused(
        "last_admin",
        "a network cannot be left without an administrator",
    )
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
