//! Cleans the note of an exclusion record before it is shown as a reason.
//!
//! What it does: trims the text, turns control and text-direction characters into `?` and cuts it to
//! the longest note the record allows. The text comes from the network, so nothing in it may reach a
//! terminal or a page as a control sequence.
//! What it doesn't do: no escaping for a particular output (HTML, shell); that is up to the caller.

use super::record::MAX_NOTE_CHARS;

/// Characters other than controls that change how text is laid out or broken into lines.
fn is_layout(c: char) -> bool {
    matches!(
        c,
        '\u{2028}'
            | '\u{2029}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{061C}'
    )
}

fn safe(c: char) -> char {
    if c.is_control() || is_layout(c) {
        '?'
    } else {
        c
    }
}

/// The reason to show for a note: trimmed, cleaned, at most `MAX_NOTE_CHARS` characters; `None` when
/// nothing is left after trimming.
pub fn reason_text(note: &str) -> Option<String> {
    let trimmed = note.trim();
    let cleaned: String = trimmed.chars().map(safe).take(MAX_NOTE_CHARS).collect();
    (!trimmed.is_empty()).then_some(cleaned)
}

#[cfg(test)]
#[path = "reason_text_tests.rs"]
mod tests;
