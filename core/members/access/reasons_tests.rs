//! Tests of the labels of admission requests (`reasons.rs`): whose note counts and which notes are shown.

use std::collections::BTreeMap;

use bytes::Bytes;

use super::*;
use crate::members::access::record::{encode, AclKind, AclValue, Via};
use crate::members::access::tally::Admission;
use crate::transport::net::contract::pid;

fn admit_note(author: PeerId, subject: PeerId, made_unix: u64, note: &str) -> ReplicaEntry {
    let value = AclValue {
        kind: AclKind::Admit,
        subject,
        via: Via::Invite,
        made_unix,
        note: note.to_string(),
    };
    ReplicaEntry {
        author,
        key: admit_key(&subject),
        value: Bytes::from(encode(&value)),
        ts_micros: 5,
    }
}

fn asking(x: PeerId, admitted: &[PeerId]) -> Ledger {
    let request = Admission { label: None };
    Ledger {
        admitted: admitted.iter().copied().collect(),
        admissions: BTreeMap::from([(x, request)]),
        ..Ledger::empty()
    }
}

#[test]
fn label_is_the_newest_note_of_an_admitted_author() {
    // given: two admitted authors noted x; a stranger noted it later
    let (a, b, stranger, x) = (pid(2), pid(3), pid(4), pid(9));
    let entries = [
        admit_note(a, x, 10, "old"),
        admit_note(b, x, 20, " Anna's laptop "),
        admit_note(stranger, x, 30, "spam"),
    ];
    let mut l = asking(x, &[a, b]);
    // when
    label_admissions(&mut l, &entries);
    // then: the newest note of the admitted wins, trimmed
    assert_eq!(l.admissions[&x].label.as_deref(), Some("Anna's laptop"));
}

#[test]
fn label_is_none_without_a_fit_note() {
    // given: a blank note and a note with a control character
    let (a, b, x) = (pid(2), pid(3), pid(9));
    let entries = [
        admit_note(a, x, 10, "  "),
        admit_note(b, x, 20, "bad\nnote"),
    ];
    let mut l = asking(x, &[a, b]);
    // when
    label_admissions(&mut l, &entries);
    // then
    assert_eq!(l.admissions[&x].label, None);
}
