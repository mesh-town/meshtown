//! Tests of the cleaning of an exclusion note (`reason_text.rs`).

use super::*;

#[test]
fn blank_note_is_none() {
    assert_eq!(reason_text(""), None);
}

#[test]
fn spaces_and_tabs_only_is_none() {
    assert_eq!(reason_text("  \t "), None);
}

#[test]
fn trims_spaces() {
    assert_eq!(reason_text("  spam \t").as_deref(), Some("spam"));
}

#[test]
fn esc_sequence_becomes_question_marks() {
    let text = reason_text("\u{1b}[31mX").unwrap();
    assert!(!text.contains('\u{1b}'));
    assert_eq!(text, "?[31mX");
}

#[test]
fn c1_csi_is_replaced() {
    assert_eq!(reason_text("a\u{9b}31mb").as_deref(), Some("a?31mb"));
}

#[test]
fn line_and_paragraph_separators_are_replaced() {
    assert_eq!(reason_text("a\u{2028}b\u{2029}c").as_deref(), Some("a?b?c"));
}

#[test]
fn bidi_override_is_replaced() {
    let all = "\u{202A}\u{202B}\u{202C}\u{202D}\u{202E}\u{2066}\u{2067}\u{2068}\u{2069}";
    let text = format!("a{all}b");
    assert_eq!(reason_text(&text).unwrap(), format!("a{}b", "?".repeat(9)));
}

#[test]
fn bidi_marks_lrm_rlm_alm_are_replaced() {
    assert_eq!(
        reason_text("a\u{200E}b\u{200F}c\u{61C}d").as_deref(),
        Some("a?b?c?d")
    );
}

#[test]
fn longer_than_80_chars_is_cut_to_80() {
    let text = reason_text(&"x".repeat(200)).unwrap();
    assert_eq!(text.chars().count(), 80);
}

#[test]
fn eighty_multibyte_chars_are_kept_whole() {
    let note = "é".repeat(80);
    assert_eq!(reason_text(&note), Some(note));
}
