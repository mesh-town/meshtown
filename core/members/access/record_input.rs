//! Builds the tally input from the entries of the shared list. Pure functions; the record format
//! itself is in `record.rs`.

use crate::transport::net::{PeerId, ReplicaEntry};

use super::authority::{power_of, PowerIn};
use super::record::{exclude_key, parse_entry, AclKind, Parsed};
use super::tally::{TallyInput, VoteIn};

/// Builds the tally input from the entries of the shared list: live records of power and exclusions
/// plus the tombstone of an own self-exclusion (leaving is irreversible). Returns the count of ignored
/// entries.
pub fn tally_input(root: PeerId, me: PeerId, entries: &[ReplicaEntry]) -> (TallyInput, usize) {
    let mut input = TallyInput {
        me,
        root,
        votes: Vec::new(),
        powers: Vec::new(),
    };
    let mut ignored = 0;
    for e in entries {
        ignored += absorb(&mut input, e);
    }
    (input, ignored)
}

/// Adds the record of power an entry carries, if any; a withdrawn one is absent.
fn push_power(input: &mut TallyInput, parsed: &Parsed) {
    let Parsed::Record {
        author,
        ts_micros,
        value,
    } = parsed
    else {
        return;
    };
    let Some(power) = power_of(value.kind, value.via) else {
        return;
    };
    input.powers.push(PowerIn {
        author: *author,
        power,
        subject: value.subject,
        ts_micros: *ts_micros,
    });
}

/// Adds one entry to the input; returns 1 when the entry is ignored.
fn absorb(input: &mut TallyInput, e: &ReplicaEntry) -> usize {
    let parsed = parse_entry(e);
    push_power(input, &parsed);
    match parsed {
        Parsed::Ignored(_) => 1,
        Parsed::Record {
            author,
            ts_micros,
            value,
        } if value.kind == AclKind::Exclude => {
            input.votes.push(VoteIn {
                voter: author,
                subject: value.subject,
                ts_micros,
            });
            0
        }
        Parsed::Withdrawn { author, key } if key == exclude_key(&author) => {
            input.votes.push(VoteIn {
                voter: author,
                subject: author,
                ts_micros: e.ts_micros,
            });
            0
        }
        _ => 0,
    }
}

#[cfg(test)]
#[path = "record_input_tests.rs"]
mod tests;
