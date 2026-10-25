//! Wording of the identity lines in the log and of the stale-revision error text.

use super::journal::{log_outcome, LogInputs};
use super::*;
use crate::config::config_file::save::SaveError;
use crate::node::output::{log_page, log_summary};

/// Prints the outcome for `report` (and `secret_written`) and returns the `config` lines it added.
fn config_lines_after(report: &FileReport, secret_written: bool) -> Vec<String> {
    let seq_before = log_summary(0).last_seq;
    log_outcome(LogInputs {
        file_summary: "x",
        report,
        unknown_paths: &[],
        retired_paths: &[],
        write_error: &None,
        kept_copy_path: &None,
        saved_flags_rev: None,
        written_flag_paths: &[],
        secret_written,
        cli_override_paths: &[],
    });
    log_page(seq_before, 4000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "config")
        .map(|l| l.text)
        .collect()
}

fn has_line(lines: &[String], fragment: &str) -> bool {
    lines.iter().any(|l| l.contains(fragment))
}

#[test]
fn stale_rev_text_names_the_app_not_a_node() {
    let text = helpers::save_error_text(&SaveError::StaleRev(7));
    assert!(text.contains("behind Mesh Town's back"), "{text}");
    assert!(text.contains("(7)"), "{text}");
}

#[test]
fn secret_written_line_speaks_of_identity() {
    let mut report = base_report(None);
    report.persistent_identity = true;
    let lines = config_lines_after(&report, true);
    assert!(
        has_line(&lines, "identity secret written to the file"),
        "{lines:?}"
    );
}

#[test]
fn declined_identity_line_says_this_run_only() {
    let mut report = base_report(None);
    report.identity_declined = true;
    let lines = config_lines_after(&report, false);
    assert!(has_line(&lines, "for this run only"), "{lines:?}");
    let line = lines
        .iter()
        .find(|l| l.contains("for this run only"))
        .unwrap();
    assert!(!line.contains("node"), "{line}");
}

#[test]
fn temporary_identity_line_says_temporary() {
    let report = base_report(None);
    let lines = config_lines_after(&report, false);
    let line = lines.iter().find(|l| l.contains("is temporary"));
    let line = line.unwrap_or_else(|| panic!("no temporary line: {lines:?}"));
    assert!(!line.contains("node"), "{line}");
}

#[test]
fn retired_mode_is_logged() {
    // given: a parsed file that held the retired `transport.mode`
    let seq_before = log_summary(0).last_seq;
    // when: the outcome is printed
    log_outcome(LogInputs {
        file_summary: "x",
        report: &base_report(None),
        unknown_paths: &[],
        retired_paths: &["config.transport.mode".to_string()],
        write_error: &None,
        kept_copy_path: &None,
        saved_flags_rev: None,
        written_flag_paths: &[],
        secret_written: false,
        cli_override_paths: &[],
    });
    // then: the config category has the warning with the path
    let lines: Vec<String> = log_page(seq_before, 4000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "config")
        .map(|l| l.text)
        .collect();
    let want = "deprecated fields ignored and dropped on the next write: transport.mode";
    assert!(has_line(&lines, want), "{lines:?}");
}
