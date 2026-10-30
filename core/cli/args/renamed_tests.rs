//! Tests of the table of replaced names (`renamed.rs`).

use super::*;
use crate::cli::test_args::to_os;

fn hint(args: &[&str]) -> Option<&'static str> {
    renamed_hint(&to_os(args)).map(|r| r.new)
}

#[test]
fn every_row_has_a_replacement() {
    for row in RENAMED {
        assert!(!row.old.is_empty(), "{row:?}");
        assert!(!row.new.trim().is_empty(), "{row:?}");
        assert!(row.old.iter().all(|w| !w.contains(' ')), "{row:?}");
    }
}

#[test]
fn flag_row_matches_only_under_its_command() {
    assert_eq!(
        hint(&["meshtown", "run", "--domain", "a.com"]),
        Some("run --site")
    );
    assert_eq!(hint(&["meshtown", "run", "--domain=a.com"]), Some("run --site"));
    assert_eq!(hint(&["meshtown", "status", "--domain", "a.com"]), None);
    assert_eq!(hint(&["meshtown", "--domain", "run"]), None);
}

#[test]
fn access_mode_row_needs_both_words() {
    assert_eq!(
        hint(&["meshtown", "access", "localhost"]),
        Some("access page localhost")
    );
    assert_eq!(hint(&["meshtown", "access"]), None);
    assert_eq!(hint(&["meshtown", "localhost"]), None);
}

#[test]
fn live_old_name_still_parses() {
    // The table only names what was replaced; a name that still parses never reaches it.
    let (cli, _) = crate::cli::test_args::parse_ok(&["keygen"]);
    assert!(matches!(
        cli.command,
        crate::cli::args::CliCommand::Keygen { .. }
    ));
    assert!(hint(&["meshtown", "keygen"]).is_some());
}
