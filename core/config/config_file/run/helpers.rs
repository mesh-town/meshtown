//! Small functions shared by `sources.rs` and `parsed.rs`.

use crate::transport::net::NodeSecret;
use std::path::{Path, PathBuf};

use super::super::save::SaveError;
use super::super::time::stamp_utc;
use super::super::FILE_NAME;
use crate::node::output::NET_NAME;

/// Returns a new node secret: 32 random bytes, as before in `node.rs`.
pub fn new_secret() -> anyhow::Result<NodeSecret> {
    let mut bytes = [0u8; 32];
    crate::members::network_key::fill_random(&mut bytes)?;
    Ok(NodeSecret::from_bytes(&bytes))
}

/// Returns the write error text for the report and the log.
pub(super) fn save_error_text(e: &SaveError) -> String {
    match e {
        SaveError::StaleRev(r) => {
            format!("the file on disk has another revision ({r}): it was edited behind {NET_NAME}'s back; write cancelled")
        }
        SaveError::Io(m) => m.clone(),
    }
}

/// Estimates whether writing is possible without writing: the file exists and is not marked read-only.
pub(super) fn is_probably_writable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| !m.permissions().readonly())
        .unwrap_or(false)
}

/// Copies (does not rename) a readable file to `config.json.bad-<stamp_utc>` next to it before
/// a write that distorts it (a corrupted secret, fields that failed their bounds). The copy gets
/// mode `0600` (Unix) and does not overwrite an earlier copy with the same name. `Err` holds text with the
/// path and reason; then the original file must not be overwritten.
pub(in super::super) fn keep_copy_before_lossy_write(
    path: &Path,
    now_unix: u64,
) -> Result<PathBuf, String> {
    let target = path.with_file_name(format!("{FILE_NAME}.bad-{}", stamp_utc(now_unix)));
    if target.exists() {
        return Err(format!(
            "copy {} already exists, not writing over it",
            target.display()
        ));
    }
    let failed = |e: std::io::Error| {
        format!(
            "couldn't save a copy of {} as {}: {e}",
            path.display(),
            target.display()
        )
    };
    std::fs::copy(path, &target).map_err(failed)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)) {
            // A copy with the network key and wider than `0600` permissions is worse than no copy.
            let _ = std::fs::remove_file(&target);
            return Err(failed(e));
        }
    }
    Ok(target)
}
