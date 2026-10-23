//! Builds the tally input from the entries of the shared list. Pure functions; the record format
//! itself is in `record.rs`.

use crate::transport::net::{PeerId, ReplicaEntry};

use super::record::{exclude_key, parse_entry, AclKind, Parsed};
use super::tally::{AdmitIn, TallyInput, VoteIn};

/// Builds the tally input from the entries of the shared list: live admits and exclusions plus the
/// tombstone of an own self-exclusion (leaving is irreversible). Returns the count of ignored entries.
pub fn tally_input(me: PeerId, entries: &[ReplicaEntry]) -> (TallyInput, usize) {
    let mut input = TallyInput {
        me,
        admits: Vec::new(),
        votes: Vec::new(),
    };
    let mut ignored = 0;
    for e in entries {
        ignored += absorb(&mut input, e);
    }
    (input, ignored)
}

/// Adds one entry to the input; returns 1 when the entry is ignored.
fn absorb(input: &mut TallyInput, e: &ReplicaEntry) -> usize {
    match parse_entry(e) {
        Parsed::Ignored(_) => 1,
        Parsed::Record { author, value, .. } if value.kind == AclKind::Admit => {
            input.admits.push(AdmitIn {
                author,
                subject: value.subject,
                via: value.via,
            });
            0
        }
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
