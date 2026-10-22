//! Member list of the demo node: five fixed members and the rules of the member actions
//! (exclusion votes, blocks, leaving). The JSON shape is built in `members_reply.rs`.

use meshtown::web::{ActionError, MemberActionReq, MemberError};

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
}

pub(crate) struct DemoMembers {
    pub(crate) me_excluded: bool,
    /// This computer asked to join and waits for confirmations.
    pub(crate) me_pending: bool,
    second_vote: bool,
    pub(crate) entries: Vec<Entry>,
}

/// The demo id of a fake member: one byte repeated, so it is a valid 64-hex id.
fn fake_id(byte: &str) -> String {
    byte.repeat(32)
}

fn entry(id_full: String, id: String, name: &str, online: bool, standing: Standing) -> Entry {
    Entry { id_full, id, name: Some(name.to_string()), online, standing, voters: Vec::new(), blocked: false, reason: None, label: None }
}

fn is_full_id(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn refused(code: &'static str, message: &str) -> MemberError {
    MemberError::Refused { code, message: message.to_string() }
}

fn target_of(act: &MemberActionReq) -> Option<&str> {
    match act {
        MemberActionReq::Exclude { id, .. }
        | MemberActionReq::Unexclude(id)
        | MemberActionReq::Admit(id)
        | MemberActionReq::Unadmit(id)
        | MemberActionReq::Block(id)
        | MemberActionReq::Unblock(id) => Some(id),
        MemberActionReq::Leave => None,
    }
}

impl DemoMembers {
    /// You, `peer-one` (ordinary), `peer-two` (somebody else's removal proposal waits for a second voter),
    /// `peer-three` (removed by votes) and `stray-laptop` (not in the list); with `--members-pending` also
    /// `anna-laptop`, which `peer-one` has confirmed.
    pub(crate) fn new(peers: &[DemoPeer], opts: &DemoNetworkOptions) -> Self {
        let (second_vote, me_excluded) = (opts.members_second_vote, opts.me_excluded);
        let mut entries = vec![entry(ME.to_string(), "".to_string(), "finch-unwary", true, Standing::Member)];
        entries.extend(peers.iter().map(|p| {
            let name = p.name.clone().unwrap_or_default();
            entry(p.id_full.to_string(), p.id.clone(), &name, p.online, Standing::Member)
        }));
        let ids: Vec<String> = entries.iter().skip(1).map(|e| e.id_full.clone()).collect();
        if let [one, _] = ids.as_slice() {
            entries[2].voters = vec![one.clone()];
            entries[2].reason = Some("lost laptop".to_string());
        }
        let gone = Exit { how: "votes", voters: ids.clone() };
        let id3 = fake_id("a3");
        entries.push(entry(id3.clone(), id3[..8].to_string(), "peer-three", false, Standing::Excluded(gone)));
        let id4 = fake_id("b4");
        entries.push(entry(id4.clone(), id4[..8].to_string(), "stray-laptop", false, Standing::Unlisted));
        if opts.members_pending {
            let mut anna = entry(fake_id("c5"), "c5c5c5c5".to_string(), "anna-laptop", true, Standing::Pending);
            anna.label = Some("Anna's laptop".to_string());
            entries.push(anna);
        }
        let mut members = DemoMembers { me_excluded: false, me_pending: opts.me_pending, second_vote, entries };
        if me_excluded {
            members.remove_me(Exit { how: "votes", voters: ids.into_iter().take(1).collect() });
        }
        members
    }

    /// Votes needed now: 1 while at most two members are in the list, otherwise 2.
    pub(crate) fn need(&self) -> u8 {
        let n = self.entries.iter().filter(|e| matches!(e.standing, Standing::Member)).count();
        if n <= 2 { 1 } else { 2 }
    }

    /// Applies an action; `Ok(changed)`.
    pub(crate) fn apply(&mut self, act: &MemberActionReq) -> Result<bool, MemberError> {
        let Some(id) = target_of(act) else { return Ok(self.leave()) };
        if self.me_excluded {
            return Err(refused("left", LOST_MSG));
        }
        if !is_full_id(id) {
            return Err(MemberError::Action(ActionError::Invalid("id must be 64 hex digits".to_string())));
        }
        let at = self.entries.iter().position(|e| e.id_full == id.to_ascii_lowercase());
        match (act, at) {
            (MemberActionReq::Unblock(_), None) => Ok(false),
            (_, None) => Err(MemberError::Action(ActionError::NotFound("no such member".to_string()))),
            (_, Some(i)) if self.entries[i].id_full == ME => Err(refused("self_target", "use Leave for this computer")),
            (MemberActionReq::Exclude { reason, .. }, Some(i)) => self.exclude(i, reason.clone()),
            (MemberActionReq::Unexclude(_), Some(i)) => Ok(self.unexclude(i)),
            (MemberActionReq::Admit(_), Some(i)) => self.admit(i),
            (MemberActionReq::Unadmit(_), Some(i)) => self.unadmit(i),
            (MemberActionReq::Block(_), Some(i)) => Ok(self.set_blocked(i, true)),
            (_, Some(i)) => Ok(self.set_blocked(i, false)),
        }
    }

    /// My confirmation of a waiting node; it joins once the threshold is reached.
    fn admit(&mut self, i: usize) -> Result<bool, MemberError> {
        let e = &mut self.entries[i];
        if !matches!(e.standing, Standing::Pending) || e.voters.iter().any(|v| v == ME) {
            return Err(refused("not_pending", "This node isn't waiting for confirmation"));
        }
        e.voters.push(ME.to_string());
        e.standing = Standing::Member;
        Ok(true)
    }

    /// Withdraws my confirmation of a node that has not joined yet.
    fn unadmit(&mut self, i: usize) -> Result<bool, MemberError> {
        let e = &mut self.entries[i];
        let mine = matches!(e.standing, Standing::Pending) && e.voters.iter().any(|v| v == ME);
        if !mine {
            return Err(refused("not_confirmed", "You haven't confirmed this node, or it has already joined"));
        }
        e.voters.retain(|v| v != ME);
        Ok(true)
    }

    fn leave(&mut self) -> bool {
        let done = !self.me_excluded;
        self.remove_me(Exit { how: "left", voters: vec![ME.to_string()] });
        done
    }

    fn remove_me(&mut self, exit: Exit) {
        self.me_excluded = true;
        self.entries[0].standing = Standing::Excluded(exit);
    }

    fn set_blocked(&mut self, i: usize, value: bool) -> bool {
        let changed = self.entries[i].blocked != value;
        self.entries[i].blocked = value;
        changed
    }

    fn exclude(&mut self, i: usize, reason: Option<String>) -> Result<bool, MemberError> {
        let e = &mut self.entries[i];
        if matches!(e.standing, Standing::Unlisted) {
            return Err(refused("not_member", "the node is not in the member list"));
        }
        if !matches!(e.standing, Standing::Member) || e.voters.iter().any(|v| v == ME) {
            return Ok(false);
        }
        if e.voters.is_empty() {
            e.reason = reason;
        }
        e.voters.push(ME.to_string());
        self.add_second_voter(i);
        self.settle();
        Ok(true)
    }

    /// With `--members-second-vote` another member agrees at once, when one more vote is still needed.
    fn add_second_voter(&mut self, i: usize) {
        if !self.second_vote || self.entries[i].voters.len() >= usize::from(self.need()) {
            return;
        }
        let have = self.entries[i].voters.clone();
        let other = self.entries.iter().enumerate().find(|(j, e)| {
            *j != i && e.id_full != ME && matches!(e.standing, Standing::Member) && !have.contains(&e.id_full)
        });
        if let Some((_, e)) = other {
            let id = e.id_full.clone();
            self.entries[i].voters.push(id);
        }
    }

    /// Members whose votes reached the threshold become excluded; the threshold may fall after each.
    fn settle(&mut self) {
        let need = usize::from(self.need());
        let hit = self.entries.iter().position(|e| {
            e.id_full != ME && matches!(e.standing, Standing::Member) && e.voters.len() >= need
        });
        let Some(i) = hit else { return };
        let voters = std::mem::take(&mut self.entries[i].voters);
        self.entries[i].standing = Standing::Excluded(Exit { how: "votes", voters });
        self.settle();
    }

    fn unexclude(&mut self, i: usize) -> bool {
        let e = &mut self.entries[i];
        if let Standing::Excluded(exit) = &mut e.standing {
            let held = exit.how == "votes" && exit.voters.iter().any(|v| v == ME);
            if !held {
                return false;
            }
            e.voters = std::mem::take(&mut exit.voters);
            e.standing = Standing::Member;
        }
        let before = e.voters.len();
        e.voters.retain(|v| v != ME);
        if e.voters.is_empty() {
            e.reason = None;
        }
        before != e.voters.len()
    }
}
