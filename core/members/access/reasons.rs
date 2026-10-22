//! The reasons of pending exclusions: what the first voter noted, when it is fit to show.
//!
//! What it does: picks, for each pending subject, the note of the first voter's `acl/exclude/<subject>`
//! record and keeps it only if it is non-blank, within the limit and free of control characters.
//! What it doesn't do: no tally and no replica reads; the entries come from the caller's scan.

use std::collections::BTreeMap;

use crate::transport::net::{PeerId, ReplicaEntry};

use super::record::{
    admit_key, exclude_key, parse_entry, AclKind, AclValue, Parsed, MAX_NOTE_CHARS,
};
use super::tally::Ledger;

/// Per pending subject, the shown reason of its proposer.
pub(super) fn reasons_of(ledger: &Ledger, entries: &[ReplicaEntry]) -> BTreeMap<PeerId, String> {
    let reason = |(subject, pending): (&PeerId, &super::tally::PendingVote)| {
        let proposer = pending.voters.first()?;
        let entry = entries
            .iter()
            .find(|e| e.author == *proposer && e.key == exclude_key(subject))?;
        Some((*subject, shown(note_of(entry)?)?))
    };
    ledger.pending.iter().filter_map(reason).collect()
}

/// Fills `Admission.label` of each request: the newest shown note among the admit records of admitted authors.
pub(super) fn label_admissions(ledger: &mut Ledger, entries: &[ReplicaEntry]) {
    let admitted = &ledger.admitted;
    let labels: Vec<(PeerId, Option<String>)> = ledger
        .admissions
        .keys()
        .map(|subject| (*subject, label_of(subject, admitted, entries)))
        .collect();
    for (subject, label) in labels {
        if let Some(a) = ledger.admissions.get_mut(&subject) {
            a.label = label;
        }
    }
}

fn label_of(
    subject: &PeerId,
    admitted: &std::collections::BTreeSet<PeerId>,
    entries: &[ReplicaEntry],
) -> Option<String> {
    let key = admit_key(subject);
    let notes = entries
        .iter()
        .filter(|e| e.key == key && admitted.contains(&e.author))
        .filter_map(admit_note);
    let newest = notes.max_by_key(|(made, _)| *made)?;
    shown(newest.1)
}

/// `(made_unix, note)` of a readable admit record with a non-blank note.
fn admit_note(entry: &ReplicaEntry) -> Option<(u64, String)> {
    match parse_entry(entry) {
        Parsed::Record {
            value:
                AclValue {
                    kind: AclKind::Admit,
                    made_unix,
                    note,
                    ..
                },
            ..
        } if !note.trim().is_empty() => Some((made_unix, note)),
        _ => None,
    }
}

fn note_of(entry: &ReplicaEntry) -> Option<String> {
    match parse_entry(entry) {
        Parsed::Record { value, .. } => Some(value.note),
        _ => None,
    }
}

/// The trimmed note when it may be shown.
fn shown(note: String) -> Option<String> {
    let text = note.trim();
    let fit = !text.is_empty()
        && text.chars().count() <= MAX_NOTE_CHARS
        && !text.chars().any(char::is_control);
    fit.then(|| text.to_string())
}

#[cfg(test)]
#[path = "reasons_tests.rs"]
mod tests;
