//! The `network` block of `/api/state`: the label, the mask, the id of the network and what this node may do
//! in it.
//!
//! What it does: joins what the key says (label, creator) with what the node knows (`NetRoles`).
//! What it doesn't do: no rules of its own — the flags come from `access::abilities`.

use serde_json::{json, Value};

use crate::members::access::abilities::{abilities_of, Abilities};
use crate::members::access::report::members_reply;
use crate::members::access::view::Verdicts;
use crate::members::membership::Members;
use crate::members::network_key::parse_user_input;
use crate::transport::net::PeerId;
use crate::web_gui::MembersReply;

use super::state::{mask_key, ConfigState};

/// What this node is and may be in its network; read from the member list of the node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetRoles {
    /// The creator of the network (full id).
    pub root: Option<String>,
    pub me_admin: bool,
    pub admins: Vec<String>,
    pub abilities: Abilities,
    /// Who removed this node from the network, when an administrator did: full id and name.
    pub excluded_by: Option<(String, String)>,
}

impl NetRoles {
    /// The roles known from the creator alone, before the node has a member list: the creator is
    /// the only admin, anyone else is a member who sees the creator as the admin.
    pub fn bare(root: &PeerId, me: &PeerId) -> NetRoles {
        let me_admin = root == me;
        NetRoles {
            root: Some(root.to_string()),
            me_admin,
            admins: vec![root.to_string()],
            abilities: abilities_of(me_admin, 1),
            excluded_by: None,
        }
    }

    /// The roles of a live node from the published verdicts.
    pub fn of(root: &PeerId, v: &Verdicts, members: &Members) -> NetRoles {
        let mut reply = members_reply(v, members, "", &[], None);
        reply.set_root(root);
        NetRoles::from_reply(&reply)
    }

    /// The roles from the member list.
    pub fn from_reply(r: &MembersReply) -> NetRoles {
        NetRoles {
            root: r.root.clone(),
            me_admin: r.me_admin,
            admins: r.admins.clone(),
            abilities: abilities_of(r.me_admin, r.admins.len()),
            excluded_by: excluded_by(r),
        }
    }

    fn role_word(&self) -> &'static str {
        match self.me_admin {
            true => "admin",
            false => "member",
        }
    }

    fn excluded_by_value(&self) -> Value {
        match &self.excluded_by {
            Some((id, name)) => json!({"id": id, "name": name}),
            None => Value::Null,
        }
    }

    /// The fields that join the `network` block.
    pub fn fields(&self) -> Vec<(&'static str, Value)> {
        let a = &self.abilities;
        vec![
            ("root", json!(self.root)),
            ("me_role", json!(self.role_word())),
            ("admins", json!(self.admins)),
            ("can_invite", json!(a.can_invite)),
            ("can_remove", json!(a.can_remove)),
            ("can_set_admins", json!(a.can_set_admins)),
            ("can_delete_network", json!(a.can_delete_network)),
            ("can_leave", json!(a.can_leave)),
            ("excluded_by", self.excluded_by_value()),
        ]
    }
}

/// The author of the exclusion of this node, when an administrator excluded it (a leave has no author):
/// the first voter of the record, with the name the list knows for it or else its short id.
fn excluded_by(r: &MembersReply) -> Option<(String, String)> {
    let mine = r.members.iter().find(|m| m.is_self)?;
    let gone = mine.excluded.as_ref().filter(|e| e.how == "removed")?;
    let by = gone.voters.first()?;
    let name = match r.members.iter().find(|m| &m.id_full == by) {
        Some(row) => row.name.clone().unwrap_or_else(|| row.id.clone()),
        None => by.chars().take(8).collect(),
    };
    Some((by.clone(), name))
}

impl ConfigState {
    /// The part of the block that the key alone gives; `null` when there is no readable key.
    pub(crate) fn network_block(&self) -> Value {
        match self.network_parts() {
            Some((value, _)) => value,
            None => Value::Null,
        }
    }

    /// The whole block: the key's part plus the roles (`None` — the node has no verdicts yet).
    pub(crate) fn network_block_with(&self, roles: Option<&NetRoles>) -> Value {
        let Some((mut value, root)) = self.network_parts() else {
            return Value::Null;
        };
        let bare = NetRoles::bare(&root, &self.secret.peer_id());
        let roles = roles.unwrap_or(&bare);
        if let Some(obj) = value.as_object_mut() {
            obj.extend(roles.fields().into_iter().map(|(k, v)| (k.to_string(), v)));
        }
        value
    }

    fn network_parts(&self) -> Option<(Value, PeerId)> {
        let key = self.current_network_key.as_ref()?;
        let input = parse_user_input(key).ok()?;
        let value = json!({
            "label": input.label,
            "masked": mask_key(key),
            "id_hex": input.key.subkeys().network_id_hex(),
        });
        Some((value, input.root))
    }
}

#[cfg(test)]
#[path = "network_block_support_tests.rs"]
mod block_support;

#[cfg(test)]
#[path = "network_block_tests.rs"]
mod block_tests;

#[cfg(test)]
#[path = "network_block_excluded_tests.rs"]
mod block_excluded_tests;
