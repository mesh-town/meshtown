//! Reading and writing the page token.

use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub(crate) const SYMBOLS: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub(crate) const TOKEN_LEN: usize = 52;

/// Builds Crockford base32 without padding on the `SYMBOLS` alphabet.
fn encoding() -> data_encoding::Encoding {
    let mut spec = data_encoding::Specification::new();
    spec.symbols.push_str(SYMBOLS);
    spec.padding = None;
    spec.encoding().expect("the token alphabet is valid")
}

pub(crate) fn is_well_formed(t: &str) -> bool {
    t.len() == TOKEN_LEN && t.chars().all(|c| SYMBOLS.contains(c))
}

pub(crate) fn generate() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    crate::members::network_key::fill_random(&mut bytes).map_err(|e| e.to_string())?;
    Ok(encoding().encode(&bytes))
}

pub(crate) fn write_atomic(dir: &Path, token: &str) -> std::io::Result<()> {
    let tmp_path = dir.join(format!(".web-token.tmp-{}", std::process::id()));

    let write_result = (|| -> std::io::Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&tmp_path)?;
        file.write_all(token.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    let final_path = dir.join(super::TOKEN_FILE_NAME);
    if let Err(e) = std::fs::rename(&tmp_path, &final_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    if let Ok(dir_file) = std::fs::File::open(dir) {
        let _ = dir_file.sync_all();
    }

    Ok(())
}

/// The log order and texts are fixed.
#[cfg(test)]
pub(crate) fn resolve_web_token(
    dir: Option<&Path>,
    fixed_token: Option<String>,
) -> Result<String, String> {
    if let Some(token) = fixed_token {
        return Ok(persist_fixed_token(dir, token));
    }
    resolve_stored_token(dir)
}

/// The node's own token: read from the file, or generated and written. Needs no fixed token, so it is
/// safe to run before the port opens.
pub(crate) fn resolve_stored_token(dir: Option<&Path>) -> Result<String, String> {
    let Some(dir) = dir else {
        crate::node::output::say(
            "web",
            "page: there is no settings file, the page token lives only in memory for this run",
        );
        return generate();
    };

    let path = dir.join(super::TOKEN_FILE_NAME);
    if let Some(candidate) = read_existing_token(&path) {
        return Ok(candidate);
    }

    write_new_token(dir, &path)
}

/// The token came from outside (for example, from the CLI) — saves it to disk when possible, but returns it
/// in any case: a write error must not get in the way of startup.
pub(crate) fn persist_fixed_token(dir: Option<&Path>, token: String) -> String {
    if let Some(dir) = dir {
        if let Err(e) = write_atomic(dir, &token) {
            let path = dir.join(super::TOKEN_FILE_NAME);
            crate::node::output::say(
                "web",
                format!(
                    "page: token not written to {} ({e}); it is valid until the end of this run",
                    path.display()
                ),
            );
        }
    }
    token
}

/// The token is already on disk and fit for use: correct shape and owner-only permissions.
fn read_existing_token(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let candidate = raw.trim();
    if !is_well_formed(candidate) {
        return None;
    }
    let perms_ok = std::fs::metadata(path)
        .map(|m| owner_only(&m))
        .unwrap_or(false);
    if perms_ok {
        return Some(candidate.to_string());
    }
    crate::node::output::say(
        "web",
        format!(
            "page: file {} was accessible to more than its owner, the token was replaced",
            path.display()
        ),
    );
    None
}

/// A new token: generates it and writes it to disk; a write error does not stop the token being returned to the caller.
fn write_new_token(dir: &Path, path: &Path) -> Result<String, String> {
    let token = generate()?;
    if let Err(e) = write_atomic(dir, &token) {
        crate::node::output::say(
            "web",
            format!(
                "page: token not written to {} ({e}); it is valid until the end of this run",
                path.display()
            ),
        );
    }
    Ok(token)
}

/// The file is accessible only to its owner. Windows has no Unix-style permissions: the file lies in the user's
/// settings directory, and access to it is set by that directory's inherited permissions.
#[cfg(unix)]
pub(super) fn owner_only(meta: &std::fs::Metadata) -> bool {
    meta.permissions().mode() & 0o077 == 0
}

#[cfg(not(unix))]
pub(super) fn owner_only(_meta: &std::fs::Metadata) -> bool {
    true
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
