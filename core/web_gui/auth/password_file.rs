//! The `web-password` password file next to `web-token` (section "On-disk format" of the sign-in plan).
//! `read_password_file` writes nothing to the log: it is called on every status, the caller writes the line.

use std::io;
use std::num::NonZeroU32;
use std::path::Path;

use serde::Deserialize;

use super::password::{PasswordRecord, MAX_FILE_ITERATIONS};
use crate::config::config_file::write_tmp_and_rename;

pub const PASSWORD_FILE_NAME: &str = "web-password";
const MAX_FILE_BYTES: u64 = 4096;
const ALG: &str = "pbkdf2-sha256";

#[derive(Debug, PartialEq)]
pub(crate) enum PasswordFileState {
    Absent,
    Unreadable(String),
    Set(PasswordRecord),
}

#[derive(Deserialize)]
struct OnDisk {
    version: u64,
    alg: String,
    iterations: u64,
    salt: String,
    hash: String,
    set_at: u64,
}

fn decode_exact<const N: usize>(text: &str, what: &str) -> Result<[u8; N], String> {
    let bytes = data_encoding::BASE64
        .decode(text.as_bytes())
        .map_err(|_| format!("{what} not base64"))?;
    bytes.try_into().map_err(|_| format!("{what} wrong size"))
}

fn into_record(d: OnDisk) -> Result<PasswordRecord, String> {
    if d.version != 1 {
        return Err(format!("format version {} is not supported", d.version));
    }
    if d.alg != ALG {
        return Err(format!("unknown algorithm \"{}\"", d.alg));
    }
    let iterations = u32::try_from(d.iterations)
        .ok()
        .filter(|n| *n <= MAX_FILE_ITERATIONS)
        .and_then(NonZeroU32::new)
        .ok_or_else(|| "iteration count out of the allowed range".to_string())?;
    Ok(PasswordRecord {
        iterations,
        salt: decode_exact(&d.salt, "salt")?,
        hash: decode_exact(&d.hash, "hash")?,
        set_at: d.set_at,
    })
}

fn parse(path: &Path) -> Result<PasswordRecord, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("read error: {e}"))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err("file is larger than 4 KiB".to_string());
    }
    if !crate::web_gui::token::owner_only(&meta) {
        return Err("permissions wider than 0600".to_string());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let d: OnDisk =
        serde_json::from_str(&text).map_err(|e| format!("not JSON of the expected format: {e}"))?;
    into_record(d)
}

pub(crate) fn read_password_file(dir: &Path) -> PasswordFileState {
    let path = dir.join(PASSWORD_FILE_NAME);
    if !path.exists() {
        return PasswordFileState::Absent;
    }
    match parse(&path) {
        Ok(rec) => PasswordFileState::Set(rec),
        Err(why) => PasswordFileState::Unreadable(why),
    }
}

pub(crate) fn write_password_file(dir: &Path, rec: &PasswordRecord) -> io::Result<()> {
    let tmp = dir.join(format!(".{PASSWORD_FILE_NAME}.tmp-{}", std::process::id()));
    let b64 = |b: &[u8]| data_encoding::BASE64.encode(b);
    let json = serde_json::json!({
        "version": 1,
        "alg": ALG,
        "iterations": rec.iterations.get(),
        "salt": b64(&rec.salt),
        "hash": b64(&rec.hash),
        "set_at": rec.set_at,
    });
    let result = write_tmp_and_rename(&tmp, &dir.join(PASSWORD_FILE_NAME), &format!("{json}\n"));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// No file is also `Ok`.
pub fn remove_password_file(dir: &Path) -> io::Result<()> {
    take_password_file(dir).map(|_| ())
}

/// Like [`remove_password_file`], and reports whether the file was there.
pub fn take_password_file(dir: &Path) -> io::Result<bool> {
    match std::fs::remove_file(dir.join(PASSWORD_FILE_NAME)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "password_file_tests.rs"]
mod tests;
