//! The member actions of the demo node: admitting, excluding, blocking and leaving. The state and the dispatch live in `members.rs`.

use meshtown_core::web_gui::MemberError;

use crate::members::{refused, DemoMembers, Exit, Standing, ME};

impl DemoMembers {
    /// Admits a waiting node at once.
    pub(crate) fn admit(&mut self, i: usize) -> Result<bool, MemberError> {
        let e = &mut self.entries[i];
        if !matches!(e.standing, Standing::Pending) {
            return Err(refused(
                "not_pending",
                "This member isn't waiting for admission",
            ));
        }
        e.standing = Standing::Member;
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

    /// Excludes a listed member at once.
    pub(crate) fn exclude(&mut self, i: usize) -> Result<bool, MemberError> {
        let e = &mut self.entries[i];
        if matches!(e.standing, Standing::Unlisted) {
            return Err(refused("not_member", "this member is not in the list"));
        }
        if !matches!(e.standing, Standing::Member) {
            return Ok(false);
        }
        e.standing = Standing::Excluded(Exit {
            how: "removed",
            voters: vec![ME.to_string()],
        });
        Ok(true)
    }

    /// Takes back my exclusion of a member.
    pub(crate) fn unexclude(&mut self, i: usize) -> bool {
        let e = &mut self.entries[i];
        let Standing::Excluded(exit) = &e.standing else {
            return false;
        };
        let mine = exit.how == "removed" && exit.voters.iter().any(|v| v == ME);
        if mine {
            e.standing = Standing::Member;
        }
        mine
    }
}
