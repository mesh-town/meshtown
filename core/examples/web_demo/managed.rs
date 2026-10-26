//! Roles of the demo network, which is always managed: the administrators, the creator (root) and the deleted
//! state, plus the refusals the real node gives for the role actions.

use meshtown_core::transport::net::{NodeSecret, PeerId};
use meshtown_core::web_gui::{MemberActionReq, MemberError};

use crate::members::{refused, DemoMembers, Standing, ME};
use crate::members_reply::demo_self_id_full;

/// What the demo plays; the network is managed in every case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DemoRole {
    /// I am the creator (root) and the only administrator.
    #[default]
    Root,
    /// I am the creator and an administrator; the first listed peer is an administrator too.
    SecondAdmin,
    /// I am an ordinary member; the two listed peers are administrators, the first one is the creator.
    Member,
}

/// Ids are the entry ids of the list, `ME` for this node.
pub(crate) struct Roles {
    pub(crate) root: String,
    pub(crate) admins: Vec<String>,
    pub(crate) closed: bool,
}

/// The node id the demo keys and invites name as the creator of the network.
pub(crate) fn demo_root() -> PeerId {
    NodeSecret::from_bytes(&[7; 32]).peer_id()
}

impl Roles {
    /// `others` are the ids of the listed peers, in list order (empty without a network).
    pub(crate) fn new(role: DemoRole, others: &[String]) -> Roles {
        let first = others.first().map(String::as_str);
        let (root, admins) = match (role, first) {
            (DemoRole::Root, _) | (DemoRole::SecondAdmin, None) | (DemoRole::Member, None) => {
                (ME, vec![ME])
            }
            (DemoRole::SecondAdmin, Some(one)) => (ME, vec![ME, one]),
            (DemoRole::Member, Some(one)) => (one, others.iter().map(String::as_str).collect()),
        };
        Roles {
            root: root.to_string(),
            admins: admins.into_iter().map(str::to_string).collect(),
            closed: false,
        }
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

impl DemoMembers {
    pub(crate) fn is_admin(&self, id: &str) -> bool {
        self.roles.admins.iter().any(|a| a == id)
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.roles.closed
    }

    fn listed(&self, id: &str) -> bool {
        match id == ME {
            true => !self.me_excluded,
            false => self.entries.iter().any(|e| {
                e.id_full == id.to_ascii_lowercase() && matches!(e.standing, Standing::Member)
            }),
        }
    }

    /// The refusals that depend on my role, before everything else.
    pub(crate) fn gate(&self, act: &MemberActionReq) -> Result<(), MemberError> {
        let roles = &self.roles;
        if roles.closed {
            return Err(refused("network_deleted", "this network has been deleted"));
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
        self.is_admin(id) && self.roles.admins.len() == 1
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
                Err(refused("not_member", "this member is not in the list"))
            }
            MemberActionReq::Revoke(id) if id == ME && self.is_last_admin(ME) => Err(last_admin()),
            _ => Ok(()),
        }
    }

    /// Grant, revoke and delete; `None` for the actions that are not about roles.
    pub(crate) fn role_act(&mut self, act: &MemberActionReq) -> Option<bool> {
        let roles = &mut self.roles;
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

fn last_admin() -> MemberError {
    refused(
        "last_admin",
        "a network cannot be left without an administrator",
    )
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
