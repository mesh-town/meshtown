//! Tests of parsing the check console commands (`console.rs`).

use super::*;

#[test]
fn console_parses_check_commands() {
    assert_eq!(
        parse_check_line("checks").unwrap().unwrap(),
        CheckCommand::Table
    );
    assert_eq!(
        parse_check_line("check example.com").unwrap().unwrap(),
        CheckCommand::Check("example.com".to_string())
    );
    assert!(parse_check_line("check").unwrap().is_err());
    assert!(parse_check_line("domains list").is_none());
    assert!(parse_check_line("").is_none());
}
