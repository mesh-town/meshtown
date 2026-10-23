//! The peers named by the invite a slot was added with, kept beside the slot's settings file as
//! `invited-peers.txt` (one peer id per line). A slot added by `run --invite` may know only the ids of
//! these peers; the node reads them back at the next start and dials them again.

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::config::config_file::write_tmp_and_rename;
use crate::transport::net::{PeerAddr, PeerId};

/// The name of the file with the invited peers.
pub const INVITED_PEERS_FILE_NAME: &str = "invited-peers.txt";

fn file_beside(slot_file: &Path) -> Option<PathBuf> {
    slot_file.parent().map(|d| d.join(INVITED_PEERS_FILE_NAME))
}

fn render(peers: &[PeerAddr]) -> String {
    let mut seen = HashSet::new();
    let ids: Vec<String> = peers
        .iter()
        .filter(|p| seen.insert(p.id))
        .map(|p| p.id.to_string())
        .collect();
    ids.join("\n") + "\n"
}

/// Writes the peer ids beside `slot_file`, one per line without repeats, in the given order. An empty
/// list writes nothing.
pub fn save_invited_peers(slot_file: &Path, peers: &[PeerAddr]) -> io::Result<()> {
    let Some(path) = file_beside(slot_file).filter(|_| !peers.is_empty()) else {
        return Ok(());
    };
    let tmp = path.with_extension(format!("txt.tmp{}", std::process::id()));
    write_tmp_and_rename(&tmp, &path, &render(peers))
}

/// Reads the peers saved beside `slot_file`. A missing or unreadable file is an empty list; lines that are
/// not peer ids are skipped.
pub fn load_invited_peers(slot_file: &Path) -> Vec<PeerAddr> {
    let text = file_beside(slot_file).and_then(|p| std::fs::read_to_string(p).ok());
    let lines = text.unwrap_or_default();
    lines
        .lines()
        .filter_map(|l| PeerId::from_str(l.trim()).ok())
        .map(PeerAddr::new)
        .collect()
}

#[cfg(test)]
#[path = "invited_tests.rs"]
mod tests;
