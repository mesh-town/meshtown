//! Member list of the demo node: five fixed members and the rules of the member actions
//! (exclusion votes, blocks, leaving). The JSON shape is built in `members_reply.rs`.

use meshtown_core::web_gui::{ActionError, MemberActionReq, MemberError};

use crate::members_forget::ForgetState;
use crate::node::{DemoNetworkOptions, DemoPeer};

/// Stands for the demo node's own id inside the list (its real id follows `boot_id`, which can change).
pub(crate) const ME: &str = "me";

const LOST_MSG: &str = "this computer is no longer in the network";

pub(crate) struct Exit {
    pub(crate) how: &'static str,
    pub(crate) voters: Vec<String>,
}

pub(crate) enum Standing {
    Member,
    /// Asked to join; `voters` holds the members that confirmed it.
    Pending,
    Unlisted,
    Excluded(Exit),
    /// Removed from the list by enough votes (see `members_forget.rs`).
    Forgotten,
}

pub(crate) struct Entry {
    pub(crate) id_full: String,
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    pub(crate) online: bool,
    pub(crate) standing: Standing,
    /// Pending exclusion votes (ids in time order); empty once the member is excluded.
    pub(crate) voters: Vec<String>,
    pub(crate) blocked: bool,
    /// Reason given with the first exclusion vote, if any.
    pub(crate) reason: Option<String>,
    /// For whom the invitation of a pending node was made.
    pub(crate) label: Option<String>,
    /// Signs of life and the circle of votes to remove the node.
    pub(crate) forget: ForgetState,
}

pub(crate) struct DemoMembers {
    pub(crate) me_excluded: bool,
    /// This computer was removed from the member list by the others (`--me-removed`).
    pub(crate) me_removed: bool,
    /// This computer asked to join and waits for confirmations.
    pub(crate) me_pending: bool,
    pub(crate) second_vote: bool,
    pub(crate) entries: Vec<Entry>,
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
        voters: Vec::new(),
        blocked: false,
        reason: None,
        label: None,
        forget: ForgetState::default(),
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
        | MemberActionReq::Unadmit(id)
        | MemberActionReq::Block(id)
        | MemberActionReq::Unblock(id)
        | MemberActionReq::Forget(id)
        | MemberActionReq::Unforget(id) => Some(id),
        MemberActionReq::Leave => None,
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
    /// You, `peer-one` (ordinary), `peer-two` (somebody else's ban proposal waits for a second voter),
    /// `peer-three` (banned by votes) and `stray-laptop` (not in the list); with `--members-pending` also
    /// `anna-laptop`, which `peer-one` has confirmed.
    pub(crate) fn new(peers: &[DemoPeer], opts: &DemoNetworkOptions) -> Self {
        let (second_vote, me_excluded) = (opts.members_second_vote, opts.me_excluded);
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
        if let [one, _] = ids.as_slice() {
            entries[2].voters = vec![one.clone()];
            entries[2].reason = Some("lost laptop".to_string());
        }
        let gone = Exit {
            how: "votes",
            voters: ids.clone(),
        };
        push_outsiders(&mut entries, gone, opts.members_pending);
        let mut members = DemoMembers {
            me_excluded: false,
            me_removed: opts.me_removed,
            me_pending: opts.me_pending,
            second_vote,
            entries,
        };
        if opts.members_forget {
            members.push_forget_rows();
        }
        if me_excluded {
            members.remove_me(Exit {
                how: "votes",
                voters: ids.into_iter().take(1).collect(),
            });
        }
        members
    }

    /// Votes needed now: 1 while at most two members are in the list, otherwise 2.
    pub(crate) fn need(&self) -> u8 {
        let n = self
            .entries
            .iter()
            .filter(|e| matches!(e.standing, Standing::Member))
            .count();
        if n <= 2 {
            1
        } else {
            2
        }
    }

    /// Applies an action; `Ok(changed)`.
    pub(crate) fn apply(&mut self, act: &MemberActionReq) -> Result<bool, MemberError> {
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
            (MemberActionReq::Exclude { reason, .. }, Some(i)) => self.exclude(i, reason.clone()),
            (MemberActionReq::Unexclude(_), Some(i)) => Ok(self.unexclude(i)),
            (MemberActionReq::Admit(_), Some(i)) => self.admit(i),
            (MemberActionReq::Unadmit(_), Some(i)) => self.unadmit(i),
            (MemberActionReq::Block(_), Some(i)) => Ok(self.set_blocked(i, true)),
            (MemberActionReq::Forget(_), Some(i)) => self.forget(i),
            (MemberActionReq::Unforget(_), Some(i)) => self.unforget(i),
            (_, Some(i)) => Ok(self.set_blocked(i, false)),
        }
    }
}
