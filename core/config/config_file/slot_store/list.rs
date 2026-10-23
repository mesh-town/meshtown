//! Reading the slot folders of `nets/`.

use std::path::{Path, PathBuf};

use super::layout::{net_slot_path, nets_dir};
use super::slot_id::NetSlotId;
use crate::config::config_file::{load_net_file, LoadedNetFile};
use crate::node::output::say;

/// One slot folder with its file read.
#[derive(Debug)]
pub struct NetSlotOnDisk {
    pub id: NetSlotId,
    pub path: PathBuf,
    pub file: LoadedNetFile,
}

/// The creation time for ordering; slots whose file did not parse go last.
fn order_key(slot: &NetSlotOnDisk) -> (bool, u64, String) {
    match &slot.file {
        LoadedNetFile::Parsed(p) => (false, p.created_unix, slot.id.to_string()),
        _ => (true, 0, slot.id.to_string()),
    }
}

/// The slot id of a directory entry: a folder named like a slot, not `main`, with a `config.json`.
fn slot_of_entry(main: &Path, entry: &std::fs::DirEntry) -> Option<NetSlotOnDisk> {
    let id = NetSlotId::parse(entry.file_name().to_str()?)?;
    let path = net_slot_path(main, &id)?;
    path.is_file().then(|| NetSlotOnDisk {
        file: load_net_file(&path),
        id,
        path,
    })
}

/// Returns the slots under `nets/` beside `main`: parsed ones by `(created_unix, id)`, the rest by id.
/// No `nets/` — empty, nothing is created; foreign names, files and set-aside folders are skipped.
pub fn list_net_slots(main: &Path) -> Vec<NetSlotOnDisk> {
    let dir = nets_dir(main);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            say(
                "config",
                format!("couldn't read network folders in {}: {e}", dir.display()),
            );
            return Vec::new();
        }
    };
    let mut slots: Vec<NetSlotOnDisk> = entries
        .filter_map(Result::ok)
        .filter_map(|e| slot_of_entry(main, &e))
        .collect();
    slots.sort_by_key(order_key);
    slots
}
