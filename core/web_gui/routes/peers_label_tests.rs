//! Tests of `check_label`: the shared check of a "for whom" note.

use super::*;

fn invalid(text: &str) -> String {
    match check_label(text) {
        Err(ActionError::Invalid(message)) => message,
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn blank_is_none() {
    assert_eq!(check_label("").unwrap(), None);
    assert_eq!(check_label("  \t ").unwrap(), None);
}

#[test]
fn text_is_trimmed() {
    assert_eq!(check_label("  Anna  ").unwrap().as_deref(), Some("Anna"));
}

#[test]
fn the_limit_is_inclusive() {
    let at_limit = "a".repeat(FOR_WHOM_MAX_CHARS);
    assert_eq!(check_label(&at_limit).unwrap(), Some(at_limit.clone()));
    let over = "a".repeat(FOR_WHOM_MAX_CHARS + 1);
    assert_eq!(
        invalid(&over),
        format!("for: at most {FOR_WHOM_MAX_CHARS} characters")
    );
}

#[test]
fn control_characters_are_invalid() {
    let message = "for: must not contain control characters";
    assert_eq!(invalid("a\nb"), message);
    assert_eq!(invalid("a\u{85}b"), message);
}

#[test]
fn line_and_paragraph_separators_are_invalid() {
    let message = "for: must not contain control characters";
    assert_eq!(invalid("a\u{2028}b"), message);
    assert_eq!(invalid("a\u{2029}b"), message);
}
