//! File sources other than a successfully parsed one (`parsed.rs`): disabled (`--no-config`),
//! no place for a file, no file, or it is unreadable/did not parse.

use crate::transport::net::NodeSecret;
use std::path::{Path, PathBuf};

use super::super::overrides::CliOverrides;
use super::super::parse::{load_config_file, LoadedConfigFile};
use super::super::report::{FileReport, FileState};
use super::super::save::{save_config_file, set_aside_bad};
use super::super::settings::Settings;
use super::fresh::create_fresh;
use super::helpers::{is_probably_writable, new_secret, save_error_text};
use super::{base_report, PrepareOutcome};

/// `ConfigSource::Disabled` (`--no-config`): a new secret for this run, the file is not touched.
pub(super) fn prepare_disabled() -> anyhow::Result<PrepareOutcome> {
    let secret = new_secret()?;
    let mut report = base_report(None);
    report.message = Some("the settings file is not used (--no-config)".to_string());
    report.identity_declined = true;
    Ok(PrepareOutcome::new(
        Settings::default(),
        report,
        "not used (--no-config)".to_string(),
        secret,
    ))
}

/// `ConfigSource::Nowhere`: no rule produced a path (no HOME/APPDATA).
pub(super) fn prepare_nowhere(why: &str) -> anyhow::Result<PrepareOutcome> {
    let secret = new_secret()?;
    let mut report = base_report(None);
    report.state = FileState::Unreadable;
    report.message = Some(why.to_string());
    let file_summary = format!("no place for the file: {why}; running on defaults and flags");
    Ok(PrepareOutcome::new(
        Settings::default(),
        report,
        file_summary,
        secret,
    ))
}

/// `ConfigSource::File`: the path is chosen, what follows depends on what is on disk.
/// A file that vanished between the path check and the read, when nobody asked to create it, is the same
/// as "unreadable".
pub(super) fn prepare_file(
    path: &Path,
    may_create: bool,
    cli: &CliOverrides,
    save_config: bool,
    now_unix: u64,
) -> anyhow::Result<PrepareOutcome> {
    let report = base_report(Some(path.to_path_buf()));
    let loaded = match load_config_file(path) {
        LoadedConfigFile::Absent if !may_create => {
            LoadedConfigFile::Unreadable("the file disappeared".to_string())
        }
        other => other,
    };
    match loaded {
        LoadedConfigFile::Absent => prepare_absent(path, cli, save_config, now_unix, report),
        LoadedConfigFile::Unreadable(reason) => prepare_broken(
            path,
            reason,
            FileState::Unreadable,
            cli,
            save_config,
            now_unix,
            report,
        ),
        LoadedConfigFile::Invalid(reason) => prepare_broken(
            path,
            reason,
            FileState::Invalid,
            cli,
            save_config,
            now_unix,
            report,
        ),
        LoadedConfigFile::Parsed(p) => {
            super::parsed::prepare_parsed(path, p, cli, save_config, now_unix, report)
        }
    }
}

/// No file: create it with defaults (and flags, if `--save-config`).
fn prepare_absent(
    path: &Path,
    cli: &CliOverrides,
    save_config: bool,
    now_unix: u64,
    mut report: FileReport,
) -> anyhow::Result<PrepareOutcome> {
    let fresh = create_fresh(path, now_unix, |s| {
        if save_config {
            cli.apply(s);
        }
    })?;
    let (file_settings, secret) = (fresh.settings, fresh.secret);
    let mut saved_flags_rev = None;
    let mut secret_written = false;
    let file_summary = match fresh.saved {
        Ok(rev) => {
            report.state = FileState::Created;
            report.rev = rev;
            report.writable = true;
            report.persistent_identity = true;
            secret_written = true;
            if save_config {
                saved_flags_rev = Some(rev);
            }
            format!("{} (created with defaults)", path.display())
        }
        Err(e) => {
            let text = format!("no file, and couldn't create it: {}", save_error_text(&e));
            report.state = FileState::Unreadable;
            report.message = Some(text.clone());
            format!(
                "{} (unreadable: {text}; running on defaults and flags, leaving the file alone)",
                path.display()
            )
        }
    };
    Ok(PrepareOutcome {
        secret_written,
        saved_flags_rev,
        ..PrepareOutcome::new(file_settings, report, file_summary, secret)
    })
}

/// The file exists but is unreadable or did not parse. `state` says which of the two: `Invalid`/`Unreadable`
/// (the "what happened" text is derived from it — both lead to the same behavior).
/// `--save-config` tries to move it aside and write a clean one.
fn prepare_broken(
    path: &Path,
    reason: String,
    state: FileState,
    cli: &CliOverrides,
    save_config: bool,
    now_unix: u64,
    mut report: FileReport,
) -> anyhow::Result<PrepareOutcome> {
    let secret = new_secret()?;
    report.state = state;
    report.message = Some(reason.clone());
    let (file_settings, moved_to, saved_flags_rev, secret_written) = if save_config {
        replace_broken_file(path, cli, now_unix, &secret, &reason, &mut report)
    } else {
        report.writable = is_probably_writable(path);
        (Settings::default(), None, None, false)
    };
    let file_summary = broken_file_summary(path, state, &report, moved_to.as_ref());
    Ok(PrepareOutcome {
        secret_written,
        saved_flags_rev,
        ..PrepareOutcome::new(file_settings, report, file_summary, secret)
    })
}

/// Returns the final line text of `prepare_broken`: the file was moved aside and created anew, or stayed
/// untouched (did not parse/unreadable) — by the same `report.message`.
fn broken_file_summary(
    path: &Path,
    state: FileState,
    report: &FileReport,
    moved_to: Option<&PathBuf>,
) -> String {
    match moved_to {
        Some(aside) => format!(
            "{} (created anew; the previous file was set aside as {})",
            path.display(),
            aside.display()
        ),
        None => format!(
            "{} ({}; running on defaults and flags, leaving the file alone)",
            path.display(),
            broken_reason_text(state, report.message.as_deref().unwrap_or(""))
        ),
    }
}

/// Returns "what happened" for a file that stayed in place: physically unreadable or did not parse.
/// A JSON syntax error ("Couldn't parse JSON: …") reads as "expected JSON, …".
fn broken_reason_text(state: FileState, message: &str) -> String {
    if state != FileState::Invalid {
        return format!("unreadable: {message}");
    }
    match message.strip_prefix("Couldn't parse JSON: ") {
        Some(position) => format!("settings file didn't parse: expected JSON, {position}"),
        None => format!("settings file didn't parse: {message}"),
    }
}

/// `--save-config` with a corrupted file: set the old one aside and write a clean file
/// with defaults and the applied flags.
fn replace_broken_file(
    path: &Path,
    cli: &CliOverrides,
    now_unix: u64,
    secret: &NodeSecret,
    reason: &str,
    report: &mut FileReport,
) -> (Settings, Option<PathBuf>, Option<u64>, bool) {
    let mut file_settings = Settings::default();
    cli.apply(&mut file_settings);
    match set_aside_bad(path, now_unix) {
        Ok(aside) => {
            let (moved_to, saved_flags_rev, secret_written) =
                save_clean_after_set_aside(path, now_unix, secret, &file_settings, &aside, report);
            (file_settings, moved_to, saved_flags_rev, secret_written)
        }
        Err(text) => {
            report.message = Some(format!("{reason}; {text}"));
            (file_settings, None, None, false)
        }
    }
}

/// Writes a clean file after the old one has been set aside; if the write fails, leaves
/// a message saying that the old file was set aside anyway.
fn save_clean_after_set_aside(
    path: &Path,
    now_unix: u64,
    secret: &NodeSecret,
    file_settings: &Settings,
    aside: &Path,
    report: &mut FileReport,
) -> (Option<PathBuf>, Option<u64>, bool) {
    match save_config_file(path, None, now_unix, secret, file_settings) {
        Ok(rev) => {
            report.state = FileState::Ok;
            report.message = Some(format!("previous file set aside as {}", aside.display()));
            report.rev = rev;
            report.writable = true;
            report.persistent_identity = true;
            (Some(aside.to_path_buf()), Some(rev), true)
        }
        Err(e) => {
            report.message = Some(format!(
                "previous file set aside as {}, but couldn't write a new one: {}",
                aside.display(),
                save_error_text(&e)
            ));
            (None, None, false)
        }
    }
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
