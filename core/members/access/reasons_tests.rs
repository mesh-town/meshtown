//! Tests of the labels of admission requests (`reasons.rs`): whose note counts and which notes are shown.

use std::collections::BTreeMap;

use bytes::Bytes;

use super::*;
use crate::members::access::record::admit_key;
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

fn admit_via(
    author: PeerId,
    subject: PeerId,
    made_unix: u64,
    via: Via,
    note: &str,
) -> ReplicaEntry {
    let mut e = admit_note(author, subject, made_unix, note);
    let value = AclValue {
        kind: AclKind::Admit,
        subject,
        via,
        made_unix,
        note: note.to_string(),
    };
    e.value = Bytes::from(encode(&value));
    e
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
    label_invited(&mut l, &entries);
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
    label_invited(&mut l, &entries);
    // then
    assert_eq!(l.admissions[&x].label, None);
}

fn admitted_only(admitted: &[PeerId]) -> Ledger {
    Ledger {
        admitted: admitted.iter().copied().collect(),
        ..Ledger::empty()
    }
}

#[test]
fn invited_as_covers_members_not_only_pending() {
    // given: an invite-admit note for a subject that is no request
    let (a, x) = (pid(2), pid(9));
    let mut l = admitted_only(&[a, x]);
    // when
    label_invited(&mut l, &[admit_note(a, x, 10, "Anna")]);
    // then
    assert_eq!(l.invited_as[&x], "Anna");
    assert!(l.admissions.is_empty());
}

#[test]
fn invited_as_skips_a_blank_note() {
    let (a, x) = (pid(2), pid(9));
    let mut l = admitted_only(&[a, x]);
    label_invited(&mut l, &[admit_note(a, x, 10, "  ")]);
    assert!(!l.invited_as.contains_key(&x));
}

#[test]
fn invited_as_ignores_a_non_admitted_author() {
    let (a, stranger, x) = (pid(2), pid(4), pid(9));
    let mut l = admitted_only(&[a, x]);
    label_invited(&mut l, &[admit_note(stranger, x, 10, "Anna")]);
    assert!(!l.invited_as.contains_key(&x));
}

#[test]
fn a_later_manual_admit_keeps_the_invite_note() {
    // given: an invite admit, then a manual one with an empty note
    let (a, b, x) = (pid(2), pid(3), pid(9));
    let mut l = admitted_only(&[a, b, x]);
    let entries = [
        admit_note(a, x, 10, "Anna"),
        admit_via(b, x, 20, Via::None, ""),
    ];
    // when
    label_invited(&mut l, &entries);
    // then
    assert_eq!(l.invited_as[&x], "Anna");
}

#[test]
fn invited_as_skips_a_non_invite_admit() {
    let (a, x) = (pid(2), pid(9));
    let mut l = admitted_only(&[a, x]);
    label_invited(&mut l, &[admit_via(a, x, 10, Via::Witness, "X")]);
    assert!(!l.invited_as.contains_key(&x));
}

#[test]
fn the_newest_invite_note_wins() {
    let (a, b, x) = (pid(2), pid(3), pid(9));
    let mut l = admitted_only(&[a, b, x]);
    let entries = [admit_note(b, x, 20, "new"), admit_note(a, x, 10, "old")];
    label_invited(&mut l, &entries);
    assert_eq!(l.invited_as[&x], "new");
}
