//! The invite strings this node made, kept beside the slot's settings file as `kept-invites.json`, so the
//! page can show an open invite again (copy, QR code). The strings are secrets: the file is owner-only, the
//! record type never prints them, and a record leaves the file when its invite is no longer open.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::config::config_file::write_tmp_and_rename;
use crate::invite::{Dealt, INVITE_TTL_SECS};

/// The name of the file with the kept invites.
pub const KEPT_INVITES_FILE_NAME: &str = "kept-invites.json";

/// Serializes read-modify-write cycles of the file within the process.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// One kept invite: when it was made and both of its strings.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptInvite {
    pub made_unix: u64,
    pub text: String,
    pub qr_text: String,
}

/// Without the strings: they hold a one-time secret.
impl std::fmt::Debug for KeptInvite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeptInvite")
            .field("made_unix", &self.made_unix)
            .finish_non_exhaustive()
    }
}

type Records = BTreeMap<String, KeptInvite>;

/// The file beside `slot_file`; `None` when it has no parent directory.
pub fn kept_invites_path_beside(slot_file: &Path) -> Option<PathBuf> {
    slot_file.parent().map(|d| d.join(KEPT_INVITES_FILE_NAME))
}

fn read(path: &Path) -> Records {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

fn write(path: &Path, records: &Records) -> io::Result<()> {
    let text = serde_json::to_string(records).map_err(io::Error::other)?;
    let tmp = path.with_extension(format!("json.tmp{}", std::process::id()));
    write_tmp_and_rename(&tmp, path, &text)
}

fn locked() -> std::sync::MutexGuard<'static, ()> {
    WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Keeps the strings of the invite named by `hash`.
pub fn keep(path: &Path, hash: &str, made_unix: u64, dealt: &Dealt) -> io::Result<()> {
    let _guard = locked();
    let mut records = read(path);
    let record = KeptInvite {
        made_unix,
        text: dealt.text.clone(),
        qr_text: dealt.qr_text.clone(),
    };
    records.insert(hash.to_string(), record);
    write(path, &records)
}

/// The kept invite under `hash`; a missing or unreadable file has none.
pub fn kept(path: &Path, hash: &str) -> Option<KeptInvite> {
    read(path).remove(hash)
}

/// The hashes that have a kept invite.
pub fn kept_hashes(path: &Path) -> BTreeSet<String> {
    read(path).into_keys().collect()
}

/// Drops every record whose hash is not in `open` or that is older than the invite lifetime; an empty
/// result removes the file. A failure is returned for the caller to log.
pub fn prune(path: &Path, open: &BTreeSet<String>, now: u64) -> io::Result<()> {
    let _guard = locked();
    let mut records = read(path);
    let before = records.len();
    let fresh = |r: &KeptInvite| r.made_unix.saturating_add(INVITE_TTL_SECS) > now;
    records.retain(|hash, r| open.contains(hash) && fresh(r));
    match (records.is_empty(), records.len() == before) {
        (true, _) => remove(path),
        (false, true) => Ok(()),
        (false, false) => write(path, &records),
    }
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "kept_invites_tests.rs"]
mod tests;
