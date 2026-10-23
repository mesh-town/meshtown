//! The file parsed (`LoadedConfigFile::Parsed`): a rewrite on data loss (`rewrite_parsed_file`) —
//! a corrupted secret and fields that failed their bounds are lost on reassembly, so before such
//! a write a copy of the file is made (`keep_copy_before_lossy_write`).

use crate::transport::net::NodeSecret;
use std::path::{Path, PathBuf};

use super::super::overrides::CliOverrides;
use super::super::parse::{ParsedConfigFile, SecretRead};
use super::super::report::{FieldError, FileReport, FileState};
use super::super::save::{is_readonly_file, readonly_refusal_text, save_config_file};
use super::super::settings::Settings;
use super::helpers::{keep_copy_before_lossy_write, new_secret, save_error_text};
use super::PrepareOutcome;
use crate::node::output::NET_NAME;

/// The file parsed: the file's network key is checked before flags are applied (the parse error text is not
/// taken — it may contain a word from the key), then a rewrite, if one is needed.
pub(super) fn prepare_parsed(
    path: &Path,
    p: ParsedConfigFile,
    cli: &CliOverrides,
    save_config: bool,
    now_unix: u64,
    mut report: FileReport,
) -> anyhow::Result<PrepareOutcome> {
    let ParsedConfigFile {
        rev,
        secret: secret_read,
        settings,
        field_errors,
        unknown_paths,
    } = p;
    let lossy = matches!(secret_read, SecretRead::Bad(_)) || !field_errors.is_empty();
    let mut file_settings = settings;
    report.field_errors = field_errors;
    if let Some(k) = &file_settings.net.network_key {
        if crate::members::network_key::parse_user_input(k).is_err() {
            report.field_errors.push(FieldError {
                field_path: "network.key".to_string(),
                message: format!("the {NET_NAME} network key in the file did not parse"),
            });
        }
    }
    if save_config {
        cli.apply(&mut file_settings);
    }
    report.state = FileState::Ok;
    report.rev = rev;
    let secret_is_new = !matches!(secret_read, SecretRead::Valid(_));
    let secret = match secret_read {
        SecretRead::Valid(k) => k,
        SecretRead::Missing | SecretRead::Bad(_) => new_secret()?,
    };
    report.persistent_identity = !secret_is_new;

    let mut outcome = PrepareOutcome::new(file_settings, report, String::new(), secret);
    outcome.unknown_paths = unknown_paths;

    if secret_is_new || save_config {
        let result = rewrite_parsed_file(RewriteCtx {
            path,
            now_unix,
            rev,
            secret: &outcome.secret,
            file_settings: &outcome.file_settings,
            lossy,
            secret_is_new,
            save_config,
        });
        apply_rewrite_result(&mut outcome, result);
    } else {
        outcome.report.writable = super::helpers::is_probably_writable(path);
    }
    outcome.file_summary = format!("{} (read)", path.display());
    Ok(outcome)
}

/// Input for `rewrite_parsed_file`: everything read from the already parsed file and the decisions above
/// (assembling this one parameter at a time would trigger `too_many_arguments`).
struct RewriteCtx<'a> {
    path: &'a Path,
    now_unix: u64,
    rev: u64,
    secret: &'a NodeSecret,
    file_settings: &'a Settings,
    lossy: bool,
    secret_is_new: bool,
    save_config: bool,
}

/// The outcome of a rewrite attempt: the readonly bit is checked before the copy and before the write — neither a temporary file
/// nor a `.bad-*` copy must appear if writing is not possible.
struct RewriteResult {
    rev: Option<u64>,
    kept_copy_path: Option<PathBuf>,
    secret_written: bool,
    saved_flags_rev: Option<u64>,
    write_error: Option<String>,
    overwrite_refused: bool,
    message: Option<String>,
}

impl RewriteResult {
    /// The write was cancelled: `kept_copy_path` is `Some` only if a copy was already made before the refusal
    /// (a refusal after it is neither read-only nor a failure of the copy itself).
    fn not_written(text: String, overwrite_refused: bool, kept_copy_path: Option<PathBuf>) -> Self {
        RewriteResult {
            rev: None,
            kept_copy_path,
            secret_written: false,
            saved_flags_rev: None,
            write_error: Some(text.clone()),
            overwrite_refused,
            message: Some(text),
        }
    }

    fn written(
        rev: u64,
        kept_copy_path: Option<PathBuf>,
        secret_is_new: bool,
        save_config: bool,
    ) -> Self {
        RewriteResult {
            rev: Some(rev),
            kept_copy_path,
            secret_written: secret_is_new,
            saved_flags_rev: if save_config { Some(rev) } else { None },
            write_error: None,
            overwrite_refused: false,
            message: None,
        }
    }

    /// The copy was made, but `save_config_file` failed: not a "refusal to overwrite" (the file was still touched — by the copy),
    /// so `overwrite_refused: false`, as in the original code.
    fn save_failed(text: String, kept_copy_path: Option<PathBuf>) -> Self {
        RewriteResult::not_written(text, false, kept_copy_path)
    }
}

/// Writes the parsed file again — if possible. Does not write and returns a refusal if the file is
/// `readonly` or if a copy could not be made before the data loss.
fn rewrite_parsed_file(ctx: RewriteCtx) -> RewriteResult {
    if is_readonly_file(ctx.path) {
        return RewriteResult::not_written(readonly_refusal_text(ctx.path), true, None);
    }
    let kept = if ctx.lossy {
        keep_copy_before_lossy_write(ctx.path, ctx.now_unix).map(Some)
    } else {
        Ok(None)
    };
    let kept_copy_path = match kept {
        Err(why) => {
            let text = format!("original file not overwritten: {why}");
            return RewriteResult::not_written(text, true, None);
        }
        Ok(kept) => kept,
    };
    match save_config_file(
        ctx.path,
        Some(ctx.rev),
        ctx.now_unix,
        ctx.secret,
        ctx.file_settings,
    ) {
        Ok(new_rev) => {
            RewriteResult::written(new_rev, kept_copy_path, ctx.secret_is_new, ctx.save_config)
        }
        Err(e) => RewriteResult::save_failed(save_error_text(&e), kept_copy_path),
    }
}

/// Applies the rewrite outcome to `outcome`. `report.rev/writable/persistent_identity` change
/// only on success — as in the original code, where only the `Ok(new_rev)` branch touched these fields.
fn apply_rewrite_result(outcome: &mut PrepareOutcome, result: RewriteResult) {
    let RewriteResult {
        rev,
        kept_copy_path,
        secret_written,
        saved_flags_rev,
        write_error,
        overwrite_refused,
        message,
    } = result;
    if let Some(rev) = rev {
        outcome.report.rev = rev;
        outcome.report.writable = true;
        outcome.report.persistent_identity = true;
    }
    outcome.report.overwrite_refused = overwrite_refused;
    outcome.report.message = message;
    outcome.write_error = write_error;
    outcome.kept_copy_path = kept_copy_path;
    outcome.secret_written = secret_written;
    outcome.saved_flags_rev = saved_flags_rev;
}
