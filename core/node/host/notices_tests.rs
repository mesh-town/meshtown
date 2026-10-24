//! Tests of the host's list of notices (`notices.rs`): a deleted network is noted once with everything the page
//! shows, the list is ordered from the oldest to the newest and bounded, and a notice goes away only by its id.

// assumed: this file is declared in `notices.rs` as `#[cfg(test)] #[path = "notices_tests.rs"] mod tests;`.
// assumed: `NoticeBook` implements `Default`; `MAX_NOTICES` is a constant of `notices.rs`; the fields of
// `Notice` are readable from this module.

use super::*;
use crate::transport::net::contract::pid;

mod push_deleted {
    use super::*;

    #[test]
    fn keeps_every_field_of_the_deletion() {
        // given
        let book = NoticeBook::default();
        // when
        let id = book.push_deleted(
            Some("alpha".to_string()),
            Some("ab12cd34".to_string()),
            &pid(2),
            Some("Dana".to_string()),
            1_700_000_000,
        );
        // then
        let notes = book.list();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0].id, id);
        assert_eq!(notes[0].kind, "net_deleted");
        assert_eq!(notes[0].label.as_deref(), Some("alpha"));
        assert_eq!(notes[0].id_hex.as_deref(), Some("ab12cd34"));
        assert_eq!(
            notes[0].by,
            pid(2).to_string(),
            "the admin in the form of the full peer id"
        );
        assert_eq!(notes[0].by_name.as_deref(), Some("Dana"));
        assert_eq!(notes[0].at_unix, 1_700_000_000);
    }

    #[test]
    fn leaves_unknown_names_empty() {
        let book = NoticeBook::default();
        book.push_deleted(None, None, &pid(2), None, 5);
        let note = book.list().remove(0);
        assert_eq!(note.label, None);
        assert_eq!(note.id_hex, None);
        assert_eq!(note.by_name, None);
    }

    #[test]
    fn numbers_the_notices_from_one_upwards() {
        let book = NoticeBook::default();
        let ids: Vec<u64> = (1..=3).map(|n| push(&book, n)).collect();
        assert_eq!(ids, [1, 2, 3]);
    }

    #[test]
    fn lists_from_the_oldest_to_the_newest() {
        let book = NoticeBook::default();
        for n in [30, 10, 20] {
            push(&book, n);
        }
        let times: Vec<u64> = book.list().iter().map(|n| n.at_unix).collect();
        assert_eq!(times, [30, 10, 20], "the order of arrival, not of time");
    }

    #[test]
    fn the_same_network_admin_and_time_give_the_same_notice() {
        // given: a deletion that was noted
        let book = NoticeBook::default();
        let first = book.push_deleted(Some("a".into()), Some("ab12".into()), &pid(2), None, 7);
        // when: the same deletion is noted again
        let again = book.push_deleted(Some("a".into()), Some("ab12".into()), &pid(2), None, 7);
        // then
        assert_eq!(again, first, "the id of the existing notice");
        assert_eq!(book.list().len(), 1);
    }

    #[test]
    fn another_time_or_admin_or_network_is_a_new_notice() {
        let book = NoticeBook::default();
        let base = book.push_deleted(None, Some("ab12".into()), &pid(2), None, 7);
        let later = book.push_deleted(None, Some("ab12".into()), &pid(2), None, 8);
        let other_admin = book.push_deleted(None, Some("ab12".into()), &pid(3), None, 7);
        let other_net = book.push_deleted(None, Some("cd34".into()), &pid(2), None, 7);
        let ids = [base, later, other_admin, other_net];
        assert_eq!(ids, [1, 2, 3, 4]);
        assert_eq!(book.list().len(), 4);
    }
}

mod limit {
    use super::*;

    #[test]
    fn keeps_the_newest_notices_up_to_the_maximum() {
        // given: the book is full
        let book = NoticeBook::default();
        let max = MAX_NOTICES as u64;
        for n in 1..=max {
            push(&book, n);
        }
        // when: one more arrives
        let newest = push(&book, max + 1);
        // then: the oldest is gone, the rest keep their order
        let ids: Vec<u64> = book.list().iter().map(|n| n.id).collect();
        assert_eq!(ids.len(), MAX_NOTICES, "{ids:?}");
        assert!(!ids.contains(&1), "the oldest was pushed out: {ids:?}");
        assert_eq!(ids.first(), Some(&2));
        assert_eq!(ids.last(), Some(&newest));
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "{ids:?}");
    }

    #[test]
    fn the_maximum_is_eight() {
        assert_eq!(MAX_NOTICES, 8);
    }

    #[test]
    fn an_id_is_not_reused_after_its_notice_was_pushed_out() {
        let book = NoticeBook::default();
        for n in 1..=(MAX_NOTICES as u64 + 1) {
            push(&book, n);
        }
        let next = push(&book, 100);
        assert_eq!(next, MAX_NOTICES as u64 + 2);
    }

    #[test]
    fn a_pushed_out_notice_cannot_be_acknowledged() {
        let book = NoticeBook::default();
        for n in 1..=(MAX_NOTICES as u64 + 1) {
            push(&book, n);
        }
        assert!(!book.ack(1), "the notice with id 1 is gone");
        assert_eq!(book.list().len(), MAX_NOTICES);
    }
}

mod ack {
    use super::*;

    #[test]
    fn removes_the_notice_with_that_id() {
        // given
        let book = NoticeBook::default();
        let first = push(&book, 1);
        // when
        let removed = book.ack(first);
        // then
        assert!(removed);
        assert!(book.list().is_empty());
    }

    #[test]
    fn leaves_the_other_notices_in_order() {
        let book = NoticeBook::default();
        let ids: Vec<u64> = (1..=3).map(|n| push(&book, n)).collect();
        assert!(book.ack(ids[1]));
        let left: Vec<u64> = book.list().iter().map(|n| n.id).collect();
        assert_eq!(left, [ids[0], ids[2]]);
    }

    #[test]
    fn a_second_ack_of_the_same_id_is_refused() {
        let book = NoticeBook::default();
        let id = push(&book, 1);
        assert!(book.ack(id));
        assert!(!book.ack(id), "nothing left to acknowledge");
    }

    #[test]
    fn an_unknown_id_is_refused_and_changes_nothing() {
        let book = NoticeBook::default();
        let id = push(&book, 1);
        assert!(!book.ack(id + 100));
        assert!(!book.ack(0));
        assert_eq!(book.list().len(), 1);
    }

    #[test]
    fn the_id_of_an_acknowledged_notice_is_not_given_again() {
        let book = NoticeBook::default();
        let id = push(&book, 1);
        book.ack(id);
        let next = push(&book, 2);
        assert!(next > id, "ids only grow: {id} then {next}");
    }
}

// --- helpers ---

/// Notes a deletion by the admin with seed 2 at time `at_unix`; returns the id of the notice.
fn push(book: &NoticeBook, at_unix: u64) -> u64 {
    book.push_deleted(
        Some("alpha".to_string()),
        Some("ab12cd34".to_string()),
        &pid(2),
        None,
        at_unix,
    )
}
