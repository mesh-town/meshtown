//! The labels of admission requests: the note an admitting administrator left, when it is fit to show.
//!
//! What it does: picks, for each requesting node, the newest note of an `acl/admit/<subject>` record written
//! by an admitted author and keeps it only if it is non-blank, within the limit and free of control characters.
//! What it doesn't do: no tally and no replica reads; the entries come from the caller's scan.

use std::collections::BTreeMap;

use crate::transport::net::{PeerId, ReplicaEntry};

use super::record::{parse_entry, AclKind, AclValue, Parsed, Via, MAX_NOTE_CHARS};
use super::tally::Ledger;

/// Fills `Ledger.invited_as` for every subject with a shown invite note, and `Admission.label` of each request
/// from it: the newest shown note among the invite-admit records of admitted authors.
pub(super) fn label_invited(ledger: &mut Ledger, entries: &[ReplicaEntry]) {
    let mut newest: BTreeMap<PeerId, (u64, String)> = BTreeMap::new();
    for entry in entries
        .iter()
        .filter(|e| ledger.admitted.contains(&e.author))
    {
        if let Some((subject, made, note)) = invite_note(entry) {
            if newest.get(&subject).is_none_or(|(m, _)| made > *m) {
                newest.insert(subject, (made, note));
            }
        }
    }
    ledger.invited_as = newest
        .into_iter()
        .filter_map(|(subject, (_, note))| Some((subject, shown(note)?)))
        .collect();
    for (subject, request) in ledger.admissions.iter_mut() {
        request.label = ledger.invited_as.get(subject).cloned();
    }
}

/// `(subject, made_unix, note)` of a readable invite-admit record with a non-blank note.
fn invite_note(entry: &ReplicaEntry) -> Option<(PeerId, u64, String)> {
    match parse_entry(entry) {
        Parsed::Record {
            value:
                AclValue {
                    kind: AclKind::Admit,
                    via: Via::Invite,
                    subject,
                    made_unix,
                    note,
                },
            ..
        } if !note.trim().is_empty() => Some((subject, made_unix, note)),
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
