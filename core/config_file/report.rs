//! The file report for the node (docs/CONFIG-FILE.md section 5.1) and the on-disk state.

use std::path::PathBuf;

/// On-disk state of the file (`config.file.state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileState {
    Ok,
    Created,
    Unreadable,
    Invalid,
}

impl FileState {
    /// `ok` | `created` | `unreadable` | `invalid`.
    pub fn as_str(self) -> &'static str {
        match self {
            FileState::Ok => "ok",
            FileState::Created => "created",
            FileState::Unreadable => "unreadable",
            FileState::Invalid => "invalid",
        }
    }
}

/// A file field that did not survive reading.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldError {
    pub field_path: String,
    pub message: String,
}

/// What the web layer returns in `file`, plus the revision.
#[derive(Clone, Debug)]
pub struct FileReport {
    pub path: Option<PathBuf>, // None — the file is not used or there is no place for it
    pub state: FileState,
    pub writable: bool,
    pub message: Option<String>,
    pub field_errors: Vec<FieldError>,
    pub rev: u64,                  // 0 — there is no file on disk
    pub persistent_identity: bool, // the secret is stored in the file
    pub identity_declined: bool,   // true only with `--no-config`: a temporary identity by request, not a failure
    /// The file was deliberately not overwritten: it parsed, but overwriting would lose data, and
    /// a `.bad-…` copy could not be made (or its name is taken). Unlike an ordinary write failure,
    /// the file is intact here and may hold the network key, so it must not be deleted.
    pub overwrite_refused: bool,
}
