//! The timeline of an administered network: records of power and exclusion votes as one ordered list.
//!
//! What it does: turns the tally input into events sorted by a total order that does not depend on the
//! order of the input.
//! What it doesn't do: no judgement whether an event counts; that is `roles.rs`.

use crate::transport::net::PeerId;

use super::super::authority::{Power, PowerIn};
use super::super::record::Via;
use super::super::tally::{TallyInput, VoteIn};

/// What an event does. The order is the order of events at one instant: an admission goes before an
/// exclusion (the removal wins), the closing goes last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Act {
    Admit,
    Grant,
    Revoke,
    Exclude,
    Left,
    Close,
}

/// A record placed on the timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Event {
    pub(super) ts: u64,
    pub(super) act: Act,
    pub(super) author: PeerId,
    pub(super) subject: PeerId,
    pub(super) via: Via,
}

/// Identity of an event in the total order.
pub(super) type EventKey = (u64, Act, PeerId, PeerId);

impl Event {
    pub(super) fn key(&self) -> EventKey {
        (self.ts, self.act, self.author, self.subject)
    }
}

fn of_power(p: &PowerIn) -> Event {
    let (act, via) = match p.power {
        Power::Admit(via) => (Act::Admit, via),
        Power::Grant => (Act::Grant, Via::None),
        Power::Revoke => (Act::Revoke, Via::None),
        Power::Close => (Act::Close, Via::None),
    };
    Event {
        ts: p.ts_micros,
        act,
        author: p.author,
        subject: p.subject,
        via,
    }
}

/// A vote of a node about itself is a departure, about another node an exclusion.
fn of_vote(v: &VoteIn) -> Event {
    let act = if v.voter == v.subject {
        Act::Left
    } else {
        Act::Exclude
    };
    Event {
        ts: v.ts_micros,
        act,
        author: v.voter,
        subject: v.subject,
        via: Via::None,
    }
}

/// All events of the input in timeline order.
pub(super) fn events(input: &TallyInput) -> Vec<Event> {
    let mut all: Vec<Event> = input.powers.iter().map(of_power).collect();
    all.extend(input.votes.iter().map(of_vote));
    all.sort_by_key(Event::key);
    all.dedup_by_key(|e| e.key());
    all
}
