//! Prepares the node start from the settings file and flags (`prepare`): assembles everything for the start.
//! (`docs/CONFIG-FILE.md`, section 5). Not here: parsing and writing the file as such (`parse.rs`,
//! `save.rs`) — this module decides whether to read/write, and prints the outcome to the log.
//!
//! Split by file: this `mod.rs` is the entry (`RunConfig`, `prepare`, `PrepareOutcome`);
//! `helpers` — small shared functions; `sources` — the three file sources other than a parsed one
//! (`Disabled`/`Nowhere`/`Absent`/broken); `parsed` — the file parsed, a rewrite when data is
//! lost; `journal` — what to say in the log after the outcome.

use crate::transport::net::NodeSecret;
use std::path::PathBuf;
use std::sync::Arc;

use super::overrides::CliOverrides;
use super::path::ConfigSource;
use super::report::FileReport;
use super::settings::Settings;
use super::slot_store::{MainFile, SettingsFile};

mod fresh;
mod helpers;
mod journal;
mod parsed;
mod sources;

pub use fresh::{create_fresh, Fresh};
pub(super) use helpers::keep_copy_before_lossy_write;
pub use helpers::new_secret;

/// The outcome of start preparation: what the node reads, with which secret, and what to say about the file.
#[derive(Clone)]
pub struct RunConfig {
    pub effective_settings: Settings,
    /// What is in the file, WITHOUT flags: the settings document for the page is built from it, not from
    /// `effective_settings`, otherwise "Save" would write the flag values to the file.
    pub file_settings: Settings,
    /// The node secret: always present; whether it is persistent is in `file_report.persistent_identity`.
    pub secret: NodeSecret,
    pub file_report: FileReport,
    /// Paths of fields given by flags and not written to the file.
    pub cli_override_paths: Vec<&'static str>,
    /// The file the settings are re-read from and written to; `None` — no file is used.
    pub file: Option<Arc<dyn SettingsFile>>,
}

/// What the file sources have in common — what `RunConfig` is assembled from after `source` is parsed.
struct PrepareOutcome {
    file_settings: Settings,
    report: FileReport,
    file_summary: String,
    unknown_paths: Vec<String>,
    write_error: Option<String>,
    kept_copy_path: Option<PathBuf>,
    secret_written: bool,
    saved_flags_rev: Option<u64>,
    secret: NodeSecret,
}

impl PrepareOutcome {
    /// An outcome without a file write: the other fields are defaults, filled in by `..` in the constructor.
    fn new(
        file_settings: Settings,
        report: FileReport,
        file_summary: String,
        secret: NodeSecret,
    ) -> Self {
        PrepareOutcome {
            file_settings,
            report,
            file_summary,
            unknown_paths: vec![],
            write_error: None,
            kept_copy_path: None,
            secret_written: false,
            saved_flags_rev: None,
            secret,
        }
    }
}

/// A report with defaults; `path` is `Some` only if a file is chosen (`ConfigSource::File`).
fn base_report(path: Option<PathBuf>) -> FileReport {
    FileReport {
        path,
        state: super::report::FileState::Ok,
        writable: false,
        message: None,
        field_errors: vec![],
        rev: 0,
        persistent_identity: false,
        identity_declined: false,
        overwrite_refused: false,
    }
}

/// Prepares the start: finds, reads and, if necessary, writes the settings file, decides which
/// secret to live with, and prints the outcome to the log. Returns an error only if
/// `fill_random` failed (then the node won't start even without a file). A corrupted, unreadable file and
/// a failed write are a `RunConfig` with a filled-in report (`docs/CONFIG-FILE.md`, section 5).
pub fn prepare(
    source: &ConfigSource,
    cli: &CliOverrides,
    save_config: bool,
    now_unix: u64,
) -> anyhow::Result<RunConfig> {
    let outcome = match source {
        ConfigSource::Disabled => sources::prepare_disabled()?,
        ConfigSource::Nowhere(why) => sources::prepare_nowhere(why)?,
        ConfigSource::File {
            path, may_create, ..
        } => sources::prepare_file(path, *may_create, cli, save_config, now_unix)?,
    };
    let PrepareOutcome {
        file_settings,
        report,
        file_summary,
        unknown_paths,
        write_error,
        kept_copy_path,
        secret_written,
        saved_flags_rev,
        secret,
    } = outcome;

    // Effective settings: what is in the file, plus flags.
    let mut effective_settings = file_settings.clone();
    let mut cli_override_paths = cli.apply(&mut effective_settings);
    let written_flag_paths: Vec<&'static str> = if saved_flags_rev.is_some() {
        std::mem::take(&mut cli_override_paths)
    } else {
        Vec::new()
    };

    journal::log_outcome(journal::LogInputs {
        file_summary: &file_summary,
        report: &report,
        unknown_paths: &unknown_paths,
        write_error: &write_error,
        kept_copy_path: &kept_copy_path,
        saved_flags_rev,
        written_flag_paths: &written_flag_paths,
        secret_written,
        cli_override_paths: &cli_override_paths,
    });

    let file: Option<Arc<dyn SettingsFile>> = match source {
        ConfigSource::File { path, .. } => Some(Arc::new(MainFile(path.clone()))),
        ConfigSource::Disabled | ConfigSource::Nowhere(_) => None,
    };

    Ok(RunConfig {
        effective_settings,
        file_settings,
        secret,
        file_report: report,
        cli_override_paths,
        file,
    })
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
#[path = "flags_tests.rs"]
mod flags_tests;

#[cfg(test)]
#[path = "backup_tests.rs"]
mod backup_tests;
