//! Tests of the reason of an exclusion in the ledger (`closure.rs`): which record's note is shown.

use crate::members::access::managed::support::Scene;
use crate::members::access::tally::{How, Ledger};
use crate::transport::net::contract::pid;

const ROOT: u8 = 1;
const A: u8 = 2;
const B: u8 = 3;
const X: u8 = 7;

/// The root admits A and B as administrators and X as a member.
fn scene() -> Scene {
    let mut scene = Scene::new(pid(ROOT));
    scene
        .admit(1, pid(ROOT), pid(A))
        .grant(2, pid(ROOT), pid(A))
        .admit(3, pid(ROOT), pid(B))
        .grant(4, pid(ROOT), pid(B))
        .admit(5, pid(ROOT), pid(X));
    scene
}

fn reason(ledger: &Ledger) -> Option<&str> {
    ledger.excluded[&pid(X)].reason.as_deref()
}

#[test]
fn removed_member_carries_the_note_as_reason() {
    let mut scene = scene();
    scene.exclude_with_note(10, pid(A), pid(X), "spam");
    let ledger = scene.ledger();
    assert_eq!(ledger.excluded[&pid(X)].how, How::Votes);
    assert_eq!(reason(&ledger), Some("spam"));
}

#[test]
fn exclusion_without_note_has_no_reason() {
    let mut scene = scene();
    scene.exclude(10, pid(A), pid(X));
    assert_eq!(reason(&scene.ledger()), None);
}

#[test]
fn first_admins_note_wins() {
    let mut scene = scene();
    scene
        .exclude_with_note(10, pid(A), pid(X), "spam")
        .exclude_with_note(11, pid(B), pid(X), "other");
    let ledger = scene.ledger();
    assert_eq!(ledger.excluded[&pid(X)].by, pid(A));
    assert_eq!(reason(&ledger), Some("spam"));
}

#[test]
fn left_member_with_a_note_has_no_reason() {
    let mut scene = scene();
    scene.exclude_with_note(10, pid(X), pid(X), "x");
    let ledger = scene.ledger();
    assert_eq!(ledger.excluded[&pid(X)].how, How::Left);
    assert_eq!(reason(&ledger), None);
}

#[test]
fn leave_after_exclude_keeps_the_reason() {
    let mut scene = scene();
    scene
        .exclude_with_note(10, pid(A), pid(X), "spam")
        .leave(11, pid(X));
    let ledger = scene.ledger();
    assert_eq!(ledger.excluded[&pid(X)].how, How::Votes);
    assert_eq!(reason(&ledger), Some("spam"));
}

#[test]
fn note_with_esc_is_cleaned_in_the_ledger() {
    let mut scene = scene();
    scene.exclude_with_note(10, pid(A), pid(X), "\u{1b}[31mred");
    assert_eq!(reason(&scene.ledger()), Some("?[31mred"));
}

#[test]
fn withdrawn_first_exclude_hands_reason_to_the_next() {
    // A's record is withdrawn, so only B's is in the input
    let mut scene = scene();
    scene.exclude_with_note(11, pid(B), pid(X), "other");
    let ledger = scene.ledger();
    assert_eq!(ledger.excluded[&pid(X)].by, pid(B));
    assert_eq!(reason(&ledger), Some("other"));
}

#[test]
fn exclude_by_a_demoted_admin_gives_way() {
    // the root takes A's role back at 8, before A's record at 10
    let mut scene = scene();
    scene
        .revoke(8, pid(ROOT), pid(A))
        .exclude_with_note(10, pid(A), pid(X), "spam")
        .exclude_with_note(11, pid(B), pid(X), "other");
    let ledger = scene.ledger();
    assert_eq!(ledger.excluded[&pid(X)].by, pid(B));
    assert_eq!(reason(&ledger), Some("other"));
}

#[test]
fn admit_after_exclude_drops_the_reason() {
    let mut scene = scene();
    scene
        .exclude_with_note(10, pid(A), pid(X), "spam")
        .admit(12, pid(ROOT), pid(X))
        .exclude(14, pid(B), pid(X));
    let ledger = scene.ledger();
    assert_eq!(ledger.excluded[&pid(X)].by, pid(B));
    assert_eq!(reason(&ledger), None);
}
