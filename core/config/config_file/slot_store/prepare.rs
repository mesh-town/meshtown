//! Building the start settings of an added slot from the main run settings and the slot's file.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::list::NetSlotOnDisk;
use super::prepare_log::{log_file_findings, log_identity};
use super::settings_file::NetFile;
use crate::config::config_file::fields::{field_by_path, Block};
use crate::config::config_file::{
    new_secret, save_net_file, FileReport, FileState, LoadedNetFile, ParsedNetFile, RunConfig,
    SaveError, SecretRead, Settings,
};
use crate::transport::net::NodeSecret;

/// The slot's identity after reading its file, and what happened on the way.
struct Identity {
    secret: NodeSecret,
    rev: u64,
    persistent: bool,
    /// A write was refused or failed; the reason, without any key value.
    problem: Option<String>,
    kept_copy: Option<PathBuf>,
}

impl Identity {
    fn temporary(
        secret: NodeSecret,
        rev: u64,
        problem: String,
        kept_copy: Option<PathBuf>,
    ) -> Self {
        Identity {
            secret,
            rev,
            persistent: false,
            problem: Some(problem),
            kept_copy,
        }
    }
}

/// The parsed file, or why the slot is not started.
fn usable(file: LoadedNetFile) -> Result<ParsedNetFile, String> {
    match file {
        LoadedNetFile::Parsed(p) => Ok(p),
        LoadedNetFile::Absent => Err("the file is missing".to_string()),
        LoadedNetFile::Unreadable(r) | LoadedNetFile::Invalid(r) => Err(r),
    }
}

/// A slot without a parsable network key is not started.
fn check_key(p: &ParsedNetFile) -> Result<(), String> {
    let key = p
        .net
        .network_key
        .as_deref()
        .ok_or("no network key in the file")?;
    crate::members::network_key::parse_user_input(key)
        .map(|_| ())
        .map_err(|_| "the network key in the file did not parse".to_string())
}

/// Writes a new secret into the slot file; a copy of the old file goes first when the rewrite loses data.
fn write_new_secret(path: &Path, p: &ParsedNetFile, secret: NodeSecret, now_unix: u64) -> Identity {
    if super::super::save::is_readonly_file(path) {
        let why = super::super::save::readonly_refusal_text(path);
        return Identity::temporary(secret, p.rev, why, None);
    }
    let lossy = matches!(p.secret, SecretRead::Bad(_)) || !p.field_errors.is_empty();
    let mut kept = None;
    if lossy {
        match super::super::run::keep_copy_before_lossy_write(path, now_unix) {
            Ok(c) => kept = Some(c),
            Err(why) => {
                return Identity::temporary(
                    secret,
                    p.rev,
                    format!("original file not overwritten: {why}"),
                    None,
                )
            }
        }
    }
    match save_net_file(path, Some(p.rev), now_unix, p.created_unix, &secret, &p.net) {
        Ok(rev) => Identity {
            secret,
            rev,
            persistent: true,
            problem: None,
            kept_copy: kept,
        },
        Err(e) => Identity::temporary(secret, p.rev, save_error_text(&e), kept),
    }
}

/// The write error as log text.
fn save_error_text(e: &SaveError) -> String {
    match e {
        SaveError::StaleRev(r) => {
            format!("the file on disk has another revision ({r}); write cancelled")
        }
        SaveError::Io(m) => m.clone(),
    }
}

/// Settles the slot's secret: a valid one stays, a missing or damaged one is replaced and the file rewritten.
fn settle_identity(path: &Path, p: &ParsedNetFile, now_unix: u64) -> Result<Identity, String> {
    if let SecretRead::Valid(k) = &p.secret {
        let kept = Identity {
            secret: k.clone(),
            rev: p.rev,
            persistent: true,
            problem: None,
            kept_copy: None,
        };
        return Ok(kept);
    }
    let secret = new_secret().map_err(|e| e.to_string())?;
    Ok(write_new_secret(path, p, secret, now_unix))
}

/// The main run's flag paths that belong to the process block; network flags do not apply to a slot.
pub(super) fn process_flag_paths(main: &RunConfig) -> Vec<&'static str> {
    let is_process =
        |p: &&'static str| field_by_path(p).is_some_and(|f| f.block() == Block::Process);
    main.cli_override_paths
        .iter()
        .copied()
        .filter(is_process)
        .collect()
}

fn report_of(path: &Path, p: &ParsedNetFile, id: &Identity) -> FileReport {
    FileReport {
        path: Some(path.to_path_buf()),
        state: FileState::Ok,
        writable: id.persistent || !super::super::save::is_readonly_file(path),
        message: id.problem.clone(),
        field_errors: p.field_errors.clone(),
        rev: id.rev,
        persistent_identity: id.persistent,
        identity_declined: false,
        overwrite_refused: false,
    }
}

/// Builds the run settings of an added slot: the process block from the main run (SOCKS off), the network
/// block from the slot file, the slot's own secret and file. `Err` means "do not start", the text is for the log.
pub fn prepare_net_slot(
    main: &RunConfig,
    slot: NetSlotOnDisk,
    now_unix: u64,
) -> Result<RunConfig, String> {
    let NetSlotOnDisk { id, path, file } = slot;
    let p = usable(file)?;
    check_key(&p)?;
    log_file_findings(&id, main.file_report.path.as_deref(), &p);
    let identity = settle_identity(&path, &p, now_unix)?;
    log_identity(
        &id,
        identity.kept_copy.as_ref(),
        identity.problem.as_deref(),
    );

    let mut process = main.effective_settings.process.clone();
    process.socks_enabled = false;
    let file_report = report_of(&path, &p, &identity);
    Ok(RunConfig {
        effective_settings: Settings {
            process,
            net: p.net.clone(),
        },
        file_settings: Settings {
            process: main.file_settings.process.clone(),
            net: p.net,
        },
        secret: identity.secret,
        file_report,
        cli_override_paths: process_flag_paths(main),
        file: Some(Arc::new(NetFile {
            path,
            created_unix: p.created_unix,
        })),
    })
}
