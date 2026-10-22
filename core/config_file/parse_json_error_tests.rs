//! Tests of the text of a JSON syntax error (`parse.rs`: `parse_text`).
//! Included from `parse.rs`: `#[cfg(test)] #[path = "parse_json_error_tests.rs"] mod json_error_tests;`.
//! Split out of `parse_tests.rs`: that file is already over the 400-line ceiling.

use super::*;

/// A JSON syntax error is reported with the line and column, without an English tail.
/// Positions follow `serde_json` 1.0.151 (`SliceRead::position_of_index`): the line counts from 1; the column is the bytes
/// from the start of the line up to and including the character where parsing stopped.
mod json_syntax_error {
    use super::*;

    const HEAD: &str = "Couldn't parse JSON: error at line";

    #[test]
    fn unclosed_object_names_line_1_column_1() {
        assert_eq!(parse_text("{").unwrap_err(), format!("{HEAD} 1, column 1"));
    }

    #[test]
    fn blank_lines_move_the_line_number() {
        assert_eq!(
            parse_text("\n\n{x").unwrap_err(),
            format!("{HEAD} 3, column 2")
        );
    }

    #[test]
    fn not_json_text_has_no_english_tail() {
        let err = parse_text("{ this is not json").unwrap_err();
        assert_eq!(err, format!("{HEAD} 1, column 3"));
        assert!(!err.contains("expected"), "{err}");
    }

    #[test]
    fn bare_word_has_no_english_tail() {
        let err = parse_text("x").unwrap_err();
        assert_eq!(err, format!("{HEAD} 1, column 1"));
        assert!(!err.contains("expected"), "{err}");
    }

    #[test]
    fn numbers_come_from_serde_json() {
        for input in ["{", "[1,", "\n{\"a\":", "{\"a\" 1}", "{} x"] {
            let e = serde_json::from_str::<serde_json::Value>(input).unwrap_err();
            let want = format!("{HEAD} {}, column {}", e.line(), e.column());
            assert_eq!(parse_text(input).unwrap_err(), want, "input {input:?}");
        }
    }
}
