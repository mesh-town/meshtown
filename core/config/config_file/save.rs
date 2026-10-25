//! Writing the settings file to disk: atomic write, revision check, setting aside a
//! corrupted file. Not here: reading and parsing (`parse.rs`), the text to write
//! (`render.rs`); the copy made before overwriting loses data lives in `run.rs` (its only
//! caller is `prepare`).

use crate::transport::net::NodeSecret;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::parse::{load_config_file, LoadedConfigFile};
use super::render::render_config_file;
use super::settings::Settings;
use super::time::stamp_utc;
use super::FILE_NAME;
#[cfg(unix)]
use crate::node::output::{say, NET_NAME};

/// Why the file was not written.
#[derive(Debug)]
pub enum SaveError {
    /// The disk holds a different revision: the file was edited behind Mesh Town's back. Holds the revision from disk.
    StaleRev(u64),
    /// The write failed. Holds the path and the reason as text.
    Io(String),
}

/// If the directory exists, warns when it is open to more than its owner; if not, creates it
/// (with mode `0700` on Unix).
fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    if dir.is_dir() {
        warn_loose_dir(dir);
        return Ok(());
    }
    create_private_dir(dir)
}

#[cfg(unix)]
fn warn_loose_dir(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(md) = std::fs::metadata(dir) {
        let mode = md.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            say(
                "config",
                format!(
                    "directory {} is accessible to more than its owner (mode 0{mode:o}) and holds the network key {NET_NAME}; set it to 0700",
                    dir.display()
                ),
            );
        }
    }
}

#[cfg(not(unix))]
fn warn_loose_dir(_dir: &Path) {}

/// `pub(crate)`: the node secret is also stored in the old `docs.redb`.
#[cfg(unix)]
pub(crate) fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
pub(crate) fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

/// Steps 3–4 of the write: a temporary file with mode `0600` and `fsync`, then a rename over the target.
/// `pub(crate)`: also called by `peers_cache::save` and `peer_names::PeerNames::save`, which are
/// outside this module.
pub(crate) fn write_tmp_and_rename(tmp: &Path, path: &Path, text: &str) -> std::io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(tmp)?;
    #[cfg(unix)]
    {
        // A temporary file may be left over from an earlier process with the same pid and different permissions.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(tmp, path)
}

/// Returns the refusal text for writing to a file marked read-only — shared by `save_with_floor`
/// and by the `run::prepare` branch, which checks the readonly bit itself, before `save_config_file`.
pub(super) fn readonly_refusal_text(path: &Path) -> String {
    format!(
        "file {} is marked read-only — write cancelled to avoid losing data",
        path.display()
    )
}

/// The file exists and is marked read-only (portable, via `Permissions::readonly()`,
/// not Unix `mode()`). No file — `false`: creating a new file is not blocked.
pub(super) fn is_readonly_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.permissions().readonly())
        .unwrap_or(false)
}

/// Writes the file. `expected_rev` is the revision the document was built from;
/// `None` — the file did not exist or is being set aside (`set_aside_bad`), so there is nothing to check against.
/// Returns the new revision.
pub fn save_config_file(
    path: &Path,
    expected_rev: Option<u64>,
    now_unix: u64,
    secret: &NodeSecret,
    s: &Settings,
) -> Result<u64, SaveError> {
    save_with_floor(path, expected_rev, 0, now_unix, secret, s)
}

/// Step 1 of the write: re-read the file and check the revision. With `expected_rev = None` the new
/// revision is not literally `1` but `rev_floor + 1` (a write "from scratch" after `set_aside_bad`/`Absent`
/// continues the revision known to the caller instead of starting from one).
fn compute_new_rev(
    expected_rev: Option<u64>,
    rev_floor: u64,
    path: &Path,
) -> Result<u64, SaveError> {
    let base = match expected_rev {
        Some(r) => current_rev_or_stale(r, path)?,
        None => rev_floor,
    };
    base.checked_add(1).ok_or_else(|| {
        SaveError::Io("file revision reached the u64 limit, can't write".to_string())
    })
}

/// Re-reads the file and checks that its revision is still `expected_rev`; returns that revision.
fn current_rev_or_stale(expected_rev: u64, path: &Path) -> Result<u64, SaveError> {
    match load_config_file(path) {
        LoadedConfigFile::Parsed(p) if p.rev != expected_rev => Err(SaveError::StaleRev(p.rev)),
        LoadedConfigFile::Parsed(_) => Ok(expected_rev),
        LoadedConfigFile::Absent => Err(SaveError::StaleRev(0)),
        LoadedConfigFile::Unreadable(m) | LoadedConfigFile::Invalid(m) => Err(SaveError::Io(
            format!("the file on disk got corrupted after reading: {m}"),
        )),
    }
}

/// Like `save_config_file`, but with `expected_rev = None` the new revision is not literally `1` but `rev_floor + 1`.
/// `save_config_file(...) = save_with_floor(..., 0, ...)`, so the old behavior (`None` → `1`) is kept
/// literally.
pub fn save_with_floor(
    path: &Path,
    expected_rev: Option<u64>,
    rev_floor: u64,
    now_unix: u64,
    secret: &NodeSecret,
    s: &Settings,
) -> Result<u64, SaveError> {
    // The file exists and is marked read-only — refuse before rebuilding the revision and before
    // the temporary file; no temp file is created at all.
    if is_readonly_file(path) {
        return Err(SaveError::Io(readonly_refusal_text(path)));
    }

    let new_rev = compute_new_rev(expected_rev, rev_floor, path)?;

    let text = render_config_file(new_rev, now_unix, secret, s);
    write_atomically(path, &text)?;
    Ok(new_rev)
}

/// The shared write: make sure the directory exists, write a temporary file next to the target, rename it over
/// the target, `fsync` the directory. On a failed write the temporary file is removed; a failed directory
/// `fsync` is only a log line, because the rename has already happened.
pub(super) fn write_atomically(path: &Path, text: &str) -> Result<(), SaveError> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let tmp = dir.join(format!("{FILE_NAME}.tmp-{}", std::process::id()));

    // Steps 2–4; on error the temporary file is removed and the revision does not grow.
    let written = ensure_dir(&dir).and_then(|()| write_tmp_and_rename(&tmp, path, text));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(SaveError::Io(format!(
            "couldn't write {}: {e}",
            path.display()
        )));
    }

    // Step 5: `fsync` of the directory. The rename has already happened, so an error here is only
    // a log line: `rev` in the file has grown, and returning `Io` would lie about that.
    #[cfg(unix)]
    {
        if let Err(e) = std::fs::File::open(&dir).and_then(|d| d.sync_all()) {
            say(
                "config",
                format!(
                    "couldn't flush directory {} to disk ({e}); the file is written",
                    dir.display()
                ),
            );
        }
    }
    Ok(())
}

/// Renames an unreadable file to `config.json.bad-<stamp_utc>` next to it.
/// Returns the new path.
pub fn set_aside_bad(path: &Path, now_unix: u64) -> Result<PathBuf, String> {
    let target = path.with_file_name(format!("{FILE_NAME}.bad-{}", stamp_utc(now_unix)));
    std::fs::rename(path, &target).map_err(|e| {
        format!(
            "couldn't set {} aside as {}: {e}",
            path.display(),
            target.display()
        )
    })?;
    Ok(target)
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
