//! Member list of the demo node: five fixed members and the rules of the member actions
//! (admission, exclusion, blocks, leaving). The JSON shape is built in `members_reply.rs`.

use meshtown_core::web_gui::{ActionError, MemberActionReq, MemberError};

use crate::managed::Roles;
use crate::node::{DemoNetworkOptions, DemoPeer};

/// Stands for the demo node's own id inside the list (its real id follows `boot_id`, which can change).
pub(crate) const ME: &str = "me";

const LOST_MSG: &str = "this computer is no longer in the network";

pub(crate) struct Exit {
    pub(crate) how: &'static str,
    /// Who removed the member: the administrators that did, or the member itself when it left.
    pub(crate) voters: Vec<String>,
}

pub(crate) enum Standing {
    Member,
    /// Asked to join and waits for an administrator.
    Pending,
    Unlisted,
    Excluded(Exit),
}

pub(crate) struct Entry {
    pub(crate) id_full: String,
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    pub(crate) online: bool,
    pub(crate) standing: Standing,
    pub(crate) blocked: bool,
    /// For whom the invitation of a pending node was made.
    pub(crate) label: Option<String>,
}

pub(crate) struct DemoMembers {
    pub(crate) me_excluded: bool,
    /// This computer asked to join and waits for confirmations.
    pub(crate) me_pending: bool,
    pub(crate) entries: Vec<Entry>,
    /// Administrators and the creator of the network.
    pub(crate) roles: Roles,
}

/// The demo id of a fake member: one byte repeated, so it is a valid 64-hex id.
pub(crate) fn fake_id(byte: &str) -> String {
    byte.repeat(32)
}

pub(crate) fn entry(
    id_full: String,
    id: String,
    name: &str,
    online: bool,
    standing: Standing,
) -> Entry {
    Entry {
        id_full,
        id,
        name: Some(name.to_string()),
        online,
        standing,
        blocked: false,
        label: None,
    }
}

fn is_full_id(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(crate) fn refused(code: &'static str, message: &str) -> MemberError {
    MemberError::Refused {
        code,
        message: message.to_string(),
    }
}

fn target_of(act: &MemberActionReq) -> Option<&str> {
    match act {
        MemberActionReq::Exclude { id, .. }
        | MemberActionReq::Unexclude(id)
        | MemberActionReq::Admit(id)
        | MemberActionReq::Block(id)
        | MemberActionReq::Unblock(id)
        | MemberActionReq::Grant(id)
        | MemberActionReq::Revoke(id) => Some(id),
        MemberActionReq::Leave | MemberActionReq::Close => None,
    }
}

/// Appends the banned member, the node that is not in the list and, if asked, the pending one.
fn push_outsiders(entries: &mut Vec<Entry>, gone: Exit, with_pending: bool) {
    let id3 = fake_id("a3");
    entries.push(entry(
        id3.clone(),
        id3[..8].to_string(),
        "peer-three",
        false,
        Standing::Excluded(gone),
    ));
    let id4 = fake_id("b4");
    entries.push(entry(
        id4.clone(),
        id4[..8].to_string(),
        "stray-laptop",
        false,
        Standing::Unlisted,
    ));
    if with_pending {
        let mut anna = entry(
            fake_id("c5"),
            "c5c5c5c5".to_string(),
            "anna-laptop",
            true,
            Standing::Pending,
        );
        anna.label = Some("Anna's laptop".to_string());
        entries.push(anna);
    }
}

impl DemoMembers {
    /// You, `peer-one` (ordinary), `peer-two`,
    /// `peer-three` (banned) and `stray-laptop` (not in the list); with `--members-pending` also
    /// `anna-laptop`, which waits for an administrator.
    pub(crate) fn new(peers: &[DemoPeer], opts: &DemoNetworkOptions) -> Self {
        let mut entries = vec![entry(
            ME.to_string(),
            "".to_string(),
            "finch-unwary",
            true,
            Standing::Member,
        )];
        entries.extend(peers.iter().map(|p| {
            let name = p.name.clone().unwrap_or_default();
            entry(
                p.id_full.to_string(),
                p.id.clone(),
                &name,
                p.online,
                Standing::Member,
            )
        }));
        let ids: Vec<String> = entries.iter().skip(1).map(|e| e.id_full.clone()).collect();
        let roles = Roles::new(opts.managed, &ids);
        let gone = Exit {
            how: "removed",
            voters: vec![roles.root.clone()],
        };
        push_outsiders(&mut entries, gone, opts.members_pending);
        let mut members = DemoMembers {
            me_excluded: false,
            me_pending: opts.me_pending,
            entries,
            roles,
        };
        if opts.me_excluded {
            members.remove_me(Exit {
                how: "removed",
                voters: ids.into_iter().take(1).collect(),
            });
        }
        members
    }

    /// Applies an action; `Ok(changed)`.
    pub(crate) fn apply(&mut self, act: &MemberActionReq) -> Result<bool, MemberError> {
        self.gate(act)?;
        if let Some(done) = self.role_act(act) {
            return Ok(done);
        }
        let Some(id) = target_of(act) else {
            return Ok(self.leave());
        };
        if self.me_excluded {
            return Err(refused("left", LOST_MSG));
        }
        if !is_full_id(id) {
            return Err(MemberError::Action(ActionError::Invalid(
                "id must be 64 hex digits".to_string(),
            )));
        }
        let at = self
            .entries
            .iter()
            .position(|e| e.id_full == id.to_ascii_lowercase());
        match (act, at) {
            (MemberActionReq::Unblock(_), None) => Ok(false),
            (_, None) => Err(MemberError::Action(ActionError::NotFound(
                "no such member".to_string(),
            ))),
            (_, Some(i)) if self.entries[i].id_full == ME => {
                Err(refused("self_target", "use Leave for this computer"))
            }
            (MemberActionReq::Exclude { .. }, Some(i)) => self.exclude(i),
            (MemberActionReq::Unexclude(_), Some(i)) => Ok(self.unexclude(i)),
            (MemberActionReq::Admit(_), Some(i)) => self.admit(i),
            (MemberActionReq::Block(_), Some(i)) => Ok(self.set_blocked(i, true)),
            (_, Some(i)) => Ok(self.set_blocked(i, false)),
        }
    }
}
