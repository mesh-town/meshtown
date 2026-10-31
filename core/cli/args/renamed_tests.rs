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
    assert_eq!(
        hint(&["meshtown", "run", "--domain=a.com"]),
        Some("run --site")
    );
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
fn replaced_name_no_longer_parses_and_has_a_hint() {
    // The table only names what was replaced; a name that still parses never reaches it.
    let refused = crate::cli::args::Cli::command().try_get_matches_from(["meshtown", "keygen"]);
    assert!(refused.is_err());
    assert_eq!(
        hint(&["meshtown", "keygen"]),
        Some("network create --key-only")
    );
    assert!(hint(&["meshtown", "network", "create"]).is_none());
}

#[test]
fn removed_run_flags_name_the_new_form() {
    for (old, new) in [
        ("--probe-interval", "run --set probe.interval_sec=<SECONDS>"),
        ("--probe-full-cycle", "run --set probe.full_cycle=true"),
        (
            "--check-interval",
            "run --set checks.interval_sec=<SECONDS>",
        ),
        ("--check-doh", "run --set checks.doh_server=<URL>"),
        ("--provider-dns", "run --set checks.provider_dns=<ADDR>"),
        ("--checks", "run --set checks.enabled=true"),
        ("--no-checks", "run --set checks.enabled=false"),
        ("--save-config", "config set <PATH> <VALUE>"),
        ("--block-domain", "run --block-site"),
    ] {
        assert_eq!(hint(&["meshtown", "run", old]), Some(new), "{old}");
    }
}
