//! The member actions of the demo node: confirming, excluding, blocking and leaving, and the vote
//! settlement that follows them. The state and the dispatch live in `members.rs`.

use meshtown_core::web_gui::MemberError;

use crate::members::{refused, DemoMembers, Exit, Standing, ME};

impl DemoMembers {
    /// My confirmation of a waiting node; it joins once the threshold is reached.
    pub(crate) fn admit(&mut self, i: usize) -> Result<bool, MemberError> {
        let e = &mut self.entries[i];
        if !matches!(e.standing, Standing::Pending) || e.voters.iter().any(|v| v == ME) {
            return Err(refused(
                "not_pending",
                "This node isn't waiting for confirmation",
            ));
        }
        e.voters.push(ME.to_string());
        e.standing = Standing::Member;
        Ok(true)
    }

    /// Withdraws my confirmation of a node that has not joined yet.
    pub(crate) fn unadmit(&mut self, i: usize) -> Result<bool, MemberError> {
        let e = &mut self.entries[i];
        let mine = matches!(e.standing, Standing::Pending) && e.voters.iter().any(|v| v == ME);
        if !mine {
            return Err(refused(
                "not_confirmed",
                "You haven't confirmed this node, or it has already joined",
            ));
        }
        e.voters.retain(|v| v != ME);
        Ok(true)
    }

    pub(crate) fn leave(&mut self) -> bool {
        let done = !self.me_excluded;
        self.remove_me(Exit {
            how: "left",
            voters: vec![ME.to_string()],
        });
        done
    }

    pub(crate) fn remove_me(&mut self, exit: Exit) {
        self.me_excluded = true;
        self.entries[0].standing = Standing::Excluded(exit);
    }

    pub(crate) fn set_blocked(&mut self, i: usize, value: bool) -> bool {
        let changed = self.entries[i].blocked != value;
        self.entries[i].blocked = value;
        changed
    }

    pub(crate) fn exclude(&mut self, i: usize, reason: Option<String>) -> Result<bool, MemberError> {
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
            *j != i
                && e.id_full != ME
                && matches!(e.standing, Standing::Member)
                && !have.contains(&e.id_full)
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
        self.entries[i].standing = Standing::Excluded(Exit {
            how: "votes",
            voters,
        });
        self.settle();
    }

    pub(crate) fn unexclude(&mut self, i: usize) -> bool {
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
