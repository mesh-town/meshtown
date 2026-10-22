//! Last known names of other network members, on disk.
//!
//! What it does: keeps in `peer-names.json` next to the settings file the last known name
//! of each other member, hands them to `Members` at node start before the first handshake
//! (`node::start::network::spawn_network`) and updates the record on every fresh name from
//! the handshake.
//! What it doesn't do: does not store names of members the node never connected to directly:
//! names of nodes without a direct connection come from the network document — `node_name::run_name_watch`;
//! we do not put them on disk, the document replica is on disk anyway; does not touch the DHT record and
//! `peers-cache.json`; read and write errors do not take the node down, only a log line through
//! `say("cache", …)`.
//! Links: `node::start::network::spawn_network` calls `peer_names_path_beside`, `PeerNames::load` and
//! `apply_to` before `build_stack`; `node::event_loop::dispatch_member_event` calls
//! `remember_member_and_save`; writing to disk goes through `config_file::write_tmp_and_rename`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::transport::net::PeerId;
use serde::{Deserialize, Serialize};

use crate::config::config_file::write_tmp_and_rename;
use crate::members::membership::Members;
use crate::members::words::trim_and_validate_node_name;
use crate::node::output::say;

/// File name; the file is kept next to the settings file.
pub const PEER_NAMES_FILE_NAME: &str = "peer-names.json";
/// Ceiling on the number of stored names — with room for dozens of network nodes.
pub const MAX_NAMES: usize = 256;
pub const PEER_NAMES_FORMAT_VERSION: u8 = 1;

/// Last known names of other members of one network. Disabled (`path = None`) if the
/// node has no settings file — then `load` does not read, `save` does not write.
#[derive(Debug, Clone)]
pub struct PeerNames {
    path: Option<PathBuf>,
    network_id: String,
    /// From the freshest name change to the oldest.
    entries: Vec<(PeerId, String)>,
}

/// File format on disk — for serde only, not exposed.
#[derive(Serialize, Deserialize)]
struct NamesFile {
    version: u8,
    network_id: String,
    peers: Vec<NameEntry>,
}

/// One file record — the identifier as a string (as `PeerId::to_string()` gives it), not bytes.
#[derive(Serialize, Deserialize)]
struct NameEntry {
    id: String,
    name: String,
}

/// `config` has no parent directory — `None`.
pub fn peer_names_path_beside(config: &Path) -> Option<PathBuf> {
    config.parent().map(|dir| dir.join(PEER_NAMES_FILE_NAME))
}

/// Reads and validates the names file for `load`: no file — an empty list without an error; bad JSON,
/// a foreign version or a foreign network — `Err` with the reason text (without the path — the caller adds it).
/// Corrupt records are skipped silently; a repeated id — the first (freshest) file record stays;
/// beyond `MAX_NAMES` — cut off.
fn read_valid(path: &Path, network_id: &str) -> Result<Vec<(PeerId, String)>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let file: NamesFile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if file.version != PEER_NAMES_FORMAT_VERSION {
        return Err(format!(
            "version {}, expected {PEER_NAMES_FORMAT_VERSION}",
            file.version
        ));
    }
    if file.network_id != network_id {
        return Err("file is for another network".to_string());
    }
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for entry in file.peers {
        let Ok(id) = entry.id.parse::<PeerId>() else {
            continue;
        };
        if !seen.insert(id) {
            continue;
        }
        let Ok(name) = trim_and_validate_node_name(&entry.name) else {
            continue;
        };
        entries.push((id, name.to_string()));
        if entries.len() >= MAX_NAMES {
            break;
        }
    }
    Ok(entries)
}

impl PeerNames {
    /// Reads the storage from disk; never fails. `path = None` or no file — an empty
    /// storage silently; a read/parse error, a foreign version or a foreign network — an empty storage
    /// and a log line. The path is kept even for an empty result: the next write goes into it.
    pub fn load(path: Option<&Path>, network_id: &str) -> PeerNames {
        let entries = match path {
            Some(path) => read_valid(path, network_id).unwrap_or_else(|e| {
                say("cache", format!("file {} not used: {e}", path.display()));
                Vec::new()
            }),
            None => Vec::new(),
        };
        PeerNames {
            path: path.map(Path::to_path_buf),
            network_id: network_id.to_string(),
            entries,
        }
    }

    /// A member's known name, if there is one.
    pub fn get(&self, id: PeerId) -> Option<&str> {
        self.entries
            .iter()
            .find(|(eid, _)| *eid == id)
            .map(|(_, name)| name.as_str())
    }

    /// Ids of all records, from fresh to old.
    pub fn ids(&self) -> Vec<PeerId> {
        self.entries.iter().map(|(id, _)| *id).collect()
    }

    /// Number of records in the storage.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The storage is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts the known names into `members` (`Members::set_name` for each record).
    /// A non-empty storage — one line `[cache] member names from peer-names.json: N`.
    /// Does not call `Members::add_verified_conn`.
    pub fn apply_to(&self, members: &Members) {
        for (id, name) in &self.entries {
            members.set_name(*id, name.clone());
        }
        if !self.entries.is_empty() {
            say(
                "cache",
                format!(
                    "member names from {PEER_NAMES_FILE_NAME}: {}",
                    self.entries.len()
                ),
            );
        }
    }

    /// Remembers a member's name. `true` — the id is new or the name differs: the record goes first, the tail
    /// beyond `MAX_NAMES` is cut off. `false` — the same name, nothing changes (the order too).
    pub fn remember(&mut self, id: PeerId, name: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|(eid, _)| *eid == id) {
            if self.entries[pos].1 == name {
                return false;
            }
            self.entries.remove(pos);
        }
        self.entries.insert(0, (id, name.to_string()));
        self.entries.truncate(MAX_NAMES);
        true
    }

    /// Removes the names of the listed members. `true` if anything was removed (the storage needs
    /// to be written to disk); it does not touch the disk itself.
    pub fn forget(&mut self, gone: &HashSet<PeerId>) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(id, _)| !gone.contains(id));
        self.entries.len() != before
    }

    /// Writes the storage to disk atomically with `0600` permissions (`config_file::write_tmp_and_rename`).
    /// A disabled storage (`path = None`) — `Ok(())` without writing. Does not create the directory.
    pub fn save(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let file = NamesFile {
            version: PEER_NAMES_FORMAT_VERSION,
            network_id: self.network_id.clone(),
            peers: self
                .entries
                .iter()
                .map(|(id, name)| NameEntry {
                    id: id.to_string(),
                    name: name.clone(),
                })
                .collect(),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("file {} not serialized: {e}", path.display()))?;
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let tmp = dir.join(format!("{PEER_NAMES_FILE_NAME}.tmp-{}", std::process::id()));
        write_tmp_and_rename(&tmp, path, &text)
            .map_err(|e| format!("file {} not written: {e}", path.display()))
    }

    /// Remembers the name of a confirmed member from `Members::name_of`; no name — does nothing.
    /// If `remember` returned `true` → `save`; a `save` error is written as the line
    /// `[cache] member names not written: …` and not propagated further.
    pub fn remember_member_and_save(&mut self, members: &Members, peer: PeerId) {
        let Some(name) = members.name_of(peer) else {
            return;
        };
        if self.remember(peer, &name) {
            if let Err(e) = self.save() {
                say("cache", format!("member names not written: {e}"));
            }
        }
    }
}

#[cfg(test)]
#[path = "peer_names_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "peer_names_forget_tests.rs"]
mod forget_tests;

#[cfg(test)]
#[path = "peer_names_golden_tests.rs"]
mod golden_tests;
