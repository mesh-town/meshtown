//! Tests of the reasons of pending exclusions (`reasons.rs`): whose note counts and which notes are shown.
//! Nodes: `a` (seed 2) is the subject; `b` (seed 3) proposed, `c` (seed 4) voted second.

use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;

use super::*;
use crate::members::access::record::{encode, AclKind, AclValue, Via};
use crate::members::access::tally::PendingVote;
use crate::transport::net::contract::pid;

fn ledger(pending: &[(PeerId, &[PeerId])]) -> Ledger {
    let pending: BTreeMap<PeerId, PendingVote> = pending
        .iter()
        .map(|(s, v)| {
            (
                *s,
                PendingVote {
                    voters: v.to_vec(),
                    need: 2,
                },
            )
        })
        .collect();
    Ledger {
        admitted: BTreeSet::new(),
        excluded: BTreeMap::new(),
        pending,
        need: 2,
        admissions: BTreeMap::new(),
        forgets: Default::default(),
        admit_unix: Default::default(),
        forget_marks: Default::default(),
        forget_need: 1,
    }
}

fn vote(author: PeerId, subject: PeerId, note: &str) -> ReplicaEntry {
    let value = AclValue {
        kind: AclKind::Exclude,
        subject,
        via: Via::None,
        made_unix: 111,
        note: note.to_string(),
    };
    ReplicaEntry {
        author,
        key: exclude_key(&subject),
        value: Bytes::from(encode(&value)),
        ts_micros: 5,
    }
}

fn reasons(pending: &[(PeerId, &[PeerId])], entries: &[ReplicaEntry]) -> BTreeMap<PeerId, String> {
    reasons_of(&ledger(pending), entries)
}

#[test]
fn the_note_of_the_proposer_is_taken_not_the_one_of_the_second_voter() {
    // given: b proposed with one note, c confirmed with another
    let (a, b, c) = (pid(2), pid(3), pid(4));
    let entries = [vote(c, a, "second"), vote(b, a, "first")];
    // when / then
    assert_eq!(
        reasons(&[(a, &[b, c])], &entries),
        BTreeMap::from([(a, "first".to_string())])
    );
}

#[test]
fn an_empty_or_blank_note_is_no_reason() {
    let (a, b) = (pid(2), pid(3));
    for note in ["", "   ", " \u{a0} "] {
        assert!(
            reasons(&[(a, &[b])], &[vote(b, a, note)]).is_empty(),
            "{note:?}"
        );
    }
}

#[test]
fn the_note_is_trimmed() {
    let (a, b) = (pid(2), pid(3));
    assert_eq!(
        reasons(&[(a, &[b])], &[vote(b, a, "  noisy link ")])[&a],
        "noisy link"
    );
}

#[test]
fn a_note_with_a_control_character_is_no_reason() {
    let (a, b) = (pid(2), pid(3));
    for note in ["a\nb", "a\tb", "\u{7}bell", "esc\u{1b}[0m"] {
        assert!(
            reasons(&[(a, &[b])], &[vote(b, a, note)]).is_empty(),
            "{note:?}"
        );
    }
}

#[test]
fn a_note_of_80_characters_is_shown() {
    let (a, b) = (pid(2), pid(3));
    let note = "é".repeat(80);
    assert_eq!(reasons(&[(a, &[b])], &[vote(b, a, &note)])[&a], note);
}

#[test]
fn a_note_that_is_too_long_to_read_is_no_reason() {
    // given: a record with an 81-character note, which the decoder ignores
    let (a, b) = (pid(2), pid(3));
    assert!(reasons(&[(a, &[b])], &[vote(b, a, &"a".repeat(81))]).is_empty());
}

#[test]
fn a_subject_without_a_record_of_the_proposer_has_no_reason() {
    // given: only c has a record; b, the proposer, has none; a withdrawn record counts as none
    let (a, b, c) = (pid(2), pid(3), pid(4));
    let gone = ReplicaEntry {
        value: Bytes::new(),
        ..vote(b, a, "x")
    };
    assert!(reasons(&[(a, &[b, c])], &[vote(c, a, "second")]).is_empty());
    assert!(reasons(&[(a, &[b])], &[gone]).is_empty());
    assert!(reasons(&[(a, &[b])], &[]).is_empty());
}

#[test]
fn notes_of_other_subjects_are_not_mixed_up() {
    // given: b proposes a and c, with different notes
    let (a, b, c) = (pid(2), pid(3), pid(4));
    let entries = [vote(b, a, "for a"), vote(b, c, "for c")];
    let got = reasons(&[(a, &[b]), (c, &[b])], &entries);
    assert_eq!(
        got,
        BTreeMap::from([(a, "for a".to_string()), (c, "for c".to_string())])
    );
}

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
    let request = crate::members::access::tally::Admission {
        voters: vec![],
        need: 2,
        label: None,
    };
    Ledger {
        admitted: admitted.iter().copied().collect(),
        admissions: BTreeMap::from([(x, request)]),
        ..ledger(&[])
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
