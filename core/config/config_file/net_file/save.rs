//! Writing a slot file: the revision check, the private directories, the shared atomic write.

use std::path::Path;

use super::super::net_settings::NetSettings;
use super::super::save::{is_readonly_file, readonly_refusal_text, write_atomically, SaveError};
use super::parse::{load_net_file, LoadedNetFile};
use super::render::render_net_file;
use crate::transport::net::NodeSecret;

/// Re-reads the file and checks the revision; returns the new one.
fn next_rev(path: &Path, expected_rev: Option<u64>) -> Result<u64, SaveError> {
    let overflow = || SaveError::Io("file revision reached the u64 limit, can't write".to_string());
    let Some(r) = expected_rev else { return Ok(1) };
    match load_net_file(path) {
        LoadedNetFile::Parsed(p) if p.rev != r => return Err(SaveError::StaleRev(p.rev)),
        LoadedNetFile::Parsed(_) => {}
        LoadedNetFile::Absent => return Err(SaveError::StaleRev(0)),
        LoadedNetFile::Unreadable(m) | LoadedNetFile::Invalid(m) => {
            return Err(SaveError::Io(format!(
                "the file on disk got corrupted after reading: {m}"
            )))
        }
    }
    r.checked_add(1).ok_or_else(overflow)
}

/// Flushes the `nets/` directory after it was created together with a slot directory.
#[cfg(unix)]
fn sync_new_nets_dir(nets: Option<&Path>, was_missing: bool) {
    if let (Some(dir), true) = (nets, was_missing) {
        let _ = std::fs::File::open(dir).and_then(|d| d.sync_all());
    }
}

#[cfg(not(unix))]
fn sync_new_nets_dir(_: Option<&Path>, _: bool) {}

/// Writes the slot file. `expected_rev` is the revision the caller read; `None` — the file is new (revision 1).
/// Returns the new revision.
pub fn save_net_file(
    path: &Path,
    expected_rev: Option<u64>,
    now_unix: u64,
    created_unix: u64,
    secret: &NodeSecret,
    net: &NetSettings,
) -> Result<u64, SaveError> {
    if is_readonly_file(path) {
        return Err(SaveError::Io(readonly_refusal_text(path)));
    }
    let new_rev = next_rev(path, expected_rev)?;
    let text = render_net_file(new_rev, now_unix, created_unix, secret, net);
    let nets = path.parent().and_then(Path::parent);
    let nets_missing = nets.is_some_and(|p| !p.as_os_str().is_empty() && !p.is_dir());
    write_atomically(path, &text)?;
    sync_new_nets_dir(nets, nets_missing);
    Ok(new_rev)
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
