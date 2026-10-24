//! Removal of offline members from the list in the demo: the rows that show it (`--members-forget`),
//! the votes of `forget` / `unforget` and the forget part of a row of the reply.

use meshtown_core::web_gui::{ForgetInfo, MemberError};

use crate::members::{entry, fake_id, refused, DemoMembers, Entry, Standing, ME};

/// When a removed row was removed (fixed, so the page text is stable).
const FORGOTTEN_AT: u64 = 1_792_300_000;
const DAY: u64 = 86_400;

/// The removal circle of one row and the signs of life of its node.
#[derive(Clone, Default)]
pub(crate) struct ForgetState {
    /// Seconds since the node was last seen; `None` for a node that was never seen.
    pub(crate) seen_ago: Option<u64>,
    /// Members that voted to remove the node (ids in time order, `ME` for this computer).
    pub(crate) voters: Vec<String>,
    /// When the node was removed; `None` while the circle waits for more votes.
    pub(crate) at: Option<u64>,
}

fn offline(id_byte: &str, name: &str, seen_ago: Option<u64>) -> Entry {
    let id = fake_id(id_byte);
    let mut e = entry(
        id.clone(),
        id[..8].to_string(),
        name,
        false,
        Standing::Member,
    );
    e.forget.seen_ago = seen_ago;
    e
}

fn short(id: &str, me_short: &str) -> String {
    if id == ME {
        me_short.to_string()
    } else {
        id[..8.min(id.len())].to_string()
    }
}

impl DemoMembers {
    /// Adds `old-desktop` (offline for 9 days), `ghost-key` (never seen), `lost-phone` (offline for 2 days,
    /// `peer-one` already proposed removing it) and `old-tablet` (removed by you and `peer-one`).
    pub(crate) fn push_forget_rows(&mut self) {
        let one = self.entries.get(1).map(|e| e.id_full.clone());
        let mut phone = offline("f8", "lost-phone", Some(2 * DAY));
        phone.forget.voters = one.iter().cloned().collect();
        let mut tablet = offline("a9", "old-tablet", Some(30 * DAY));
        tablet.standing = Standing::Forgotten;
        tablet.forget.voters = [ME.to_string()].into_iter().chain(one).collect();
        tablet.forget.at = Some(FORGOTTEN_AT);
        self.entries
            .push(offline("d6", "old-desktop", Some(9 * DAY)));
        self.entries.push(offline("e7", "ghost-key", None));
        self.entries.push(phone);
        self.entries.push(tablet);
    }

    /// Adds my vote to remove the row `i`; `Ok(changed)`.
    pub(crate) fn forget(&mut self, i: usize) -> Result<bool, MemberError> {
        let e = &self.entries[i];
        if !matches!(e.standing, Standing::Member) {
            return Err(refused("not_member", "the node is not in the list"));
        }
        if e.online {
            return Err(refused("online", "The node is connected right now"));
        }
        if e.forget.voters.iter().any(|v| v == ME) {
            return Ok(false);
        }
        self.entries[i].forget.voters.push(ME.to_string());
        self.settle_forgets();
        Ok(true)
    }

    /// Removes members whose circle reached the threshold; the threshold may fall after each.
    fn settle_forgets(&mut self) {
        let need = usize::from(self.need());
        let hit = self.entries.iter().position(|e| {
            matches!(e.standing, Standing::Member) && e.forget.voters.len() >= need.max(1)
        });
        let Some(i) = hit else { return };
        self.entries[i].standing = Standing::Forgotten;
        self.entries[i].forget.at = Some(FORGOTTEN_AT);
        self.settle_forgets();
    }

    /// Withdraws my vote; a removed row without enough votes is a member again; `Ok(changed)`.
    pub(crate) fn unforget(&mut self, i: usize) -> Result<bool, MemberError> {
        if !self.entries[i].forget.voters.iter().any(|v| v == ME) {
            return Err(refused(
                "not_forgotten",
                "You haven't proposed removing this node",
            ));
        }
        let need = usize::from(self.need());
        let e = &mut self.entries[i];
        e.forget.voters.retain(|v| v != ME);
        if matches!(e.standing, Standing::Forgotten) && e.forget.voters.len() < need {
            e.standing = Standing::Member;
            e.forget.at = None;
        }
        Ok(true)
    }
}

/// The circle of the row, if it has one; ids are short, as the node reports them.
pub(crate) fn forget_info(m: &DemoMembers, e: &Entry, me_short: &str) -> Option<ForgetInfo> {
    let first = e.forget.voters.first()?;
    Some(ForgetInfo {
        votes: e.forget.voters.len(),
        need: m.need(),
        proposed_by: short(first, me_short),
        voters: e.forget.voters.iter().map(|v| short(v, me_short)).collect(),
        mine: e.forget.voters.iter().any(|v| v == ME),
        at_unix: e.forget.at,
    })
}

/// Seconds since the last sign of life: zero while online.
pub(crate) fn last_seen(e: &Entry) -> Option<u64> {
    if e.online {
        return Some(0);
    }
    e.forget.seen_ago
}

#[cfg(test)]
#[path = "members_forget_tests.rs"]
mod tests;
