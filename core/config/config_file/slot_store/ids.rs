//! A new slot id: six random base32 characters that no folder under `nets/` uses.

use std::path::Path;

use anyhow::bail;

use super::layout::{nets_dir, LEFT_PREFIX};
use super::slot_id::NetSlotId;

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
const ID_LEN: usize = 6;
const ATTEMPTS: usize = 16;

/// Six random characters of the id alphabet.
fn random_id_text() -> anyhow::Result<String> {
    let mut bytes = [0u8; ID_LEN];
    crate::members::network_key::fill_random(&mut bytes)?;
    Ok(bytes
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(b % 32)]))
        .collect())
}

/// Whether `nets/<id>` or a set-aside `nets/left-<id>-*` exists.
fn is_taken(main: &Path, id: &NetSlotId) -> bool {
    let dir = nets_dir(main);
    if dir.join(id.as_str()).exists() {
        return true;
    }
    let prefix = format!("{LEFT_PREFIX}{id}-");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return false;
    };
    entries
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().starts_with(&prefix))
}

/// Picks an id with candidates from `next`; a candidate that is not a slot id or is taken costs one attempt.
pub(super) fn new_slot_id_with(
    main: &Path,
    mut next: impl FnMut() -> String,
) -> anyhow::Result<NetSlotId> {
    for _ in 0..ATTEMPTS {
        let id = NetSlotId::parse(&next()).filter(|id| *id != NetSlotId::main());
        match id {
            Some(id) if !is_taken(main, &id) => return Ok(id),
            _ => {}
        }
    }
    bail!("couldn't find a free network folder name in {ATTEMPTS} attempts")
}

/// Returns a new slot id that is free under `nets/` beside `main`.
pub fn new_slot_id(main: &Path) -> anyhow::Result<NetSlotId> {
    let mut random_failure = None;
    let picked = new_slot_id_with(main, || {
        random_id_text().unwrap_or_else(|e| {
            random_failure = Some(e);
            String::new()
        })
    });
    match random_failure {
        Some(e) => Err(e),
        None => picked,
    }
}
