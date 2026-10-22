//! Tests for the final log line for a corrupted settings file (`sources.rs`: `broken_file_summary`,
//! `prepare_file`) and for retired keys in the `prepare_file` outcome. Included from `sources.rs`:
//! `#[cfg(test)] #[path = "sources_tests.rs"] mod tests;`.

use super::*;
use crate::config_file::test_support::{scratch, zero_secret_b64};
use crate::config_file::FILE_NAME;

const NOW: u64 = 1_790_278_867;
const PATH: &str = "/tmp/x.json";
const TAIL: &str = "running on defaults and flags, leaving the file alone";

/// The file exists but did not parse as JSON, or for other document reasons.
mod invalid_file {
    use super::*;

    #[test]
    fn json_syntax_error_says_expected_json_with_position() {
        let file_summary = file_summary(
            FileState::Invalid,
            "Couldn't parse JSON: error at line 1, column 2",
        );
        assert_eq!(
            file_summary,
            format!("{PATH} (settings file didn't parse: expected JSON, error at line 1, column 2; {TAIL})")
        );
    }

    #[test]
    fn object_expected_is_shown_as_is() {
        let file_summary = file_summary(FileState::Invalid, "expected a JSON object");
        assert_eq!(
            file_summary,
            format!("{PATH} (settings file didn't parse: expected a JSON object; {TAIL})")
        );
    }

    #[test]
    fn newer_version_is_shown_as_is() {
        let why = "config.version 2 is newer than this program (1)";
        assert_eq!(
            file_summary(FileState::Invalid, why),
            format!("{PATH} (settings file didn't parse: {why}; {TAIL})")
        );
    }

    #[test]
    fn not_utf8_is_shown_as_is() {
        assert_eq!(
            file_summary(FileState::Invalid, "not UTF-8"),
            format!("{PATH} (settings file didn't parse: not UTF-8; {TAIL})")
        );
    }

    #[test]
    fn file_on_disk_gets_error_position() {
        let dir = scratch("sources-invalid");
        let file = dir.join(FILE_NAME);
        std::fs::write(&file, "{").expect("write");
        let outcome = prepare_file(&file, true, &CliOverrides::default(), false, NOW);
        let _ = std::fs::remove_dir_all(&dir);
        let want = format!(
            "{} (settings file didn't parse: expected JSON, error at line 1, column 1; {TAIL})",
            file.display()
        );
        assert_eq!(outcome.expect("prepare_file").file_summary, want);
    }
}

/// The file is unreadable from disk: the branch is unchanged.
mod unreadable_file {
    use super::*;

    // The doubling "unreadable: unreadable" is known and not fixed: the test pins
    // the current text (`load_config_file` puts "unreadable: …" in the message, `broken_kind_text` adds it once more).
    #[test]
    fn keeps_current_text() {
        assert_eq!(
            file_summary(FileState::Unreadable, "unreadable: x"),
            format!("{PATH} (unreadable: unreadable: x; {TAIL})")
        );
    }
}

/// The file was moved aside and created anew: the branch is unchanged.
mod moved_aside {
    use super::*;

    #[test]
    fn says_created_anew() {
        let report = report_with(FileState::Invalid, "anything");
        let aside = PathBuf::from("/tmp/x.json.bad-1");
        let file_summary = broken_file_summary(Path::new(PATH), FileState::Invalid, &report, Some(&aside));
        assert_eq!(
            file_summary,
            "/tmp/x.json (created anew; the previous file was set aside as /tmp/x.json.bad-1)"
        );
    }
}

/// A file with a real secret carries retired keys: they go into the outcome as a separate list.
mod retired_keys_in_file {
    use super::*;

    #[test]
    fn reach_prepare_outcome_apart_from_unknown() {
        // given: a file with a secret, where transport.udp_port and transport.loss_time_threshold
        let dir = scratch("sources-retired");
        let file = dir.join(FILE_NAME);
        let text = format!(
            r#"{{"identity":{{"secret":"{}"}},"config":{{"version":1,"transport":{{"udp_port":4000,"loss_time_threshold":2.5}}}}}}"#,
            zero_secret_b64()
        );
        std::fs::write(&file, text).expect("write");
        // when: prepare_file
        let outcome = prepare_file(&file, true, &CliOverrides::default(), false, NOW);
        let _ = std::fs::remove_dir_all(&dir);
        // then: both keys are in retired in list order, and not in unknown
        let outcome = outcome.expect("prepare_file");
        assert_eq!(
            outcome.retired_paths,
            vec!["config.transport.udp_port", "config.transport.loss_time_threshold"]
        );
        assert!(outcome.unknown_paths.is_empty(), "unknown fields: {:?}", outcome.unknown_paths);
    }
}

// ---- helpers ----

fn report_with(state: FileState, message: &str) -> FileReport {
    let mut report = base_report(Some(PathBuf::from(PATH)));
    report.state = state;
    report.message = Some(message.to_string());
    report
}

/// The final log line for a file without moving it aside.
fn file_summary(state: FileState, message: &str) -> String {
    let report = report_with(state, message);
    broken_file_summary(Path::new(PATH), state, &report, None)
}
