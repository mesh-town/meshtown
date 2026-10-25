//! What to say in the log after `prepare` (`crate::node::output::say`, category `"config"`).
//! The network secret is never printed; in path lists only the field name.

use std::path::PathBuf;

use super::super::report::FileReport;
use crate::node::output::{say, NET_NAME};

/// Inputs of one summary line: assembled in `mod.rs` from `PrepareOutcome` and the applied
/// flags, only printing happens here.
pub(super) struct LogInputs<'a> {
    pub(super) file_summary: &'a str,
    pub(super) report: &'a FileReport,
    pub(super) unknown_paths: &'a [String],
    pub(super) write_error: &'a Option<String>,
    pub(super) kept_copy_path: &'a Option<PathBuf>,
    pub(super) saved_flags_rev: Option<u64>,
    pub(super) written_flag_paths: &'a [&'static str],
    pub(super) secret_written: bool,
    pub(super) cli_override_paths: &'a [&'static str],
}

/// Prints the `prepare` outcome to the log, line by line — as before at the end of the function.
pub(super) fn log_outcome(inputs: LogInputs) {
    let LogInputs {
        file_summary,
        report,
        unknown_paths,
        write_error,
        kept_copy_path,
        saved_flags_rev,
        written_flag_paths,
        secret_written,
        cli_override_paths,
    } = inputs;

    say("config", format!("config: {file_summary}"));
    log_field_errors(report);
    log_unknown_fields(unknown_paths);
    log_write_failed(write_error);
    log_copy_kept(kept_copy_path);
    log_flags_written(saved_flags_rev, written_flag_paths);
    log_secret_written(secret_written);
    log_identity_status(report);
    log_cli_overrides(cli_override_paths);
}

/// One line for each corrupted field of the file.
fn log_field_errors(report: &FileReport) {
    for fe in &report.field_errors {
        say("config", format!("field {}: {}", fe.field_path, fe.message));
    }
}

/// Unknown fields will disappear on the next file rewrite — we warn in advance.
fn log_unknown_fields(unknown_paths: &[String]) {
    if unknown_paths.is_empty() {
        return;
    }
    say(
        "config",
        format!(
            "unknown fields skipped and will disappear on the next write: {}",
            unknown_paths.join(", ")
        ),
    );
}

/// Writing the file failed — the error text has already been prepared by the caller.
fn log_write_failed(write_error: &Option<String>) {
    let Some(text) = write_error else {
        return;
    };
    say("config", format!("couldn't write the file: {text}"));
}

/// The original file was saved next to it under another name — we print only the name, not the whole path.
fn log_copy_kept(kept_copy_path: &Option<PathBuf>) {
    let Some(kept) = kept_copy_path else {
        return;
    };
    let name = kept
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| kept.display().to_string());
    say("config", format!("config: original file saved as {name}"));
}

/// The flags revision was written — a list of the given flags or a note that there were none.
fn log_flags_written(saved_flags_rev: Option<u64>, written_flag_paths: &[&'static str]) {
    let Some(rev) = saved_flags_rev else {
        return;
    };
    let list = if written_flag_paths.is_empty() {
        "none given".to_string()
    } else {
        written_flag_paths.join(", ")
    };
    say("config", format!("flags written: {list}, revision {rev}"));
}

/// The identity secret was written to the file — a separate line so it doesn't get lost among the others.
fn log_secret_written(secret_written: bool) {
    if secret_written {
        say("config", "identity secret written to the file");
    }
}

/// This computer's identity is temporary (for one run or with no file at all) — warns that after a
/// restart this computer will join the network with a different identity.
fn log_identity_status(report: &FileReport) {
    if report.identity_declined {
        say(
            "config",
            format!("this computer's identity is for this run only (--no-config): after a restart this computer will join the {NET_NAME} network with a different identity"),
        );
    } else if !report.persistent_identity {
        say(
            "config",
            format!("this computer's identity is temporary: after a restart this computer will join the {NET_NAME} network with a different identity"),
        );
    }
}

/// CLI flags applied on top of the file for this run only.
fn log_cli_overrides(cli_override_paths: &[&'static str]) {
    if cli_override_paths.is_empty() {
        return;
    }
    say(
        "config",
        format!(
            "flags on top of the file for this run: {}",
            cli_override_paths.join(", ")
        ),
    );
}
