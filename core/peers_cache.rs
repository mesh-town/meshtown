//! Cache of the last working addresses of network members, on disk.
//!
//! What it does: keeps in `peers-cache.json` next to the settings file the addresses of members with whom
//! there has already been a working connection (`MemberEvent::Up`/`Down`), and hands them out at node start so as not to
//! wait for the DHT.
//! What it doesn't do: does not read or write the DHT, does not check that the stored addresses are alive — a member
//! may have moved during the downtime; read and write errors do not take the node down, only a log line
//! through `say("cache", …)`.
//! Links: `node::start::entry::wire_entry_points` calls `peers_cache_path_beside`, `PeersCache::load`,
//! `addrs` and `seen`; `node::event_loop::dispatch_member_event` calls `entry_for`, `remember`,
//! `touch`, `save`; writing to disk goes through `config_file::write_tmp_and_rename`.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use crate::net::{AddrHint, PeerAddr, PeerId, RelayRef};
use serde::{Deserialize, Serialize};

use crate::config_file::write_tmp_and_rename;
use crate::output::say;
use crate::net_mainline::record::{entry_to_addr, RosterEntry};

/// File name of the cache; the file is kept next to the settings file.
pub const PEERS_CACHE_FILE_NAME: &str = "peers-cache.json";
/// Ceiling on the number of members in the cache — leaves room, together with the invite addresses,
/// in the capacity of the `found_tx` channel.
pub const MAX_PEERS: usize = 32;
/// Ceiling on addresses in one record — like `net_mainline::record::MAX_ADDRS_PER_ENTRY`.
pub const MAX_ADDRS: usize = 4;
/// Lifetime of a member record without confirmation: 30 days.
pub const TTL_SECS: u64 = 30 * 24 * 3600;
/// How often the confirmation of an already known member is refreshed without changes: 1 hour.
pub const REFRESH_SECS: u64 = 3600;
pub const PEERS_CACHE_FORMAT_VERSION: u8 = 1;

/// Cache of the addresses of members of one network. Disabled (`path = None`) if the node has no settings file —
/// then `load` does not read, `save` does not write.
#[derive(Debug)]
pub struct PeersCache {
    path: Option<PathBuf>,
    network_id: String,
    peers: Vec<RosterEntry>,
}

/// File format on disk — for serde only, not exposed.
#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u8,
    network_id: String,
    peers: Vec<RosterEntry>,
}

/// `config` has no parent directory — `None`.
pub fn peers_cache_path_beside(config: &Path) -> Option<PathBuf> {
    config.parent().map(|dir| dir.join(PEERS_CACHE_FILE_NAME))
}

/// Reads and validates the cache file for `load`: no file — an empty list without an error; bad JSON,
/// a foreign version or a foreign network — `Err` with the reason text (without the path — the caller adds it).
fn read_valid(path: &Path, network_id: &str) -> Result<Vec<RosterEntry>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let file: CacheFile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if file.version != PEERS_CACHE_FORMAT_VERSION {
        return Err(format!("version {}, expected {PEERS_CACHE_FORMAT_VERSION}", file.version));
    }
    if file.network_id != network_id {
        return Err("file is for another network".to_string());
    }
    Ok(file.peers)
}

/// Relay URL from a hint; `Relay(Peer)` and `Opaque` do not go into the cache.
fn relay_url_of_hint(hint: &AddrHint) -> Option<String> {
    match hint {
        AddrHint::Relay(RelayRef::Url(url)) => Some(url.clone()),
        _ => None,
    }
}

/// A direct address from a hint.
fn direct_addr_of_hint(hint: &AddrHint) -> Option<SocketAddr> {
    match hint {
        AddrHint::Ip(addr) => Some(*addr),
        _ => None,
    }
}

/// Builds a member record: the chosen connection path (if any) goes first — `Relay` into the
/// `relay` field, `Ip` first in `addrs`; the rest (`Relay(Peer)`, `Opaque`) is skipped.
/// The remainder is filled from `known` without repeats, up to `MAX_ADDRS`: first relay (if not yet
/// taken; the first `Url` in hint order is used), then IPv4, then IPv6.
pub fn entry_for(
    known: Option<&PeerAddr>,
    peer: PeerId,
    selected: Option<&AddrHint>,
    now_unix: u64,
) -> RosterEntry {
    let mut relay = selected.and_then(relay_url_of_hint);
    let mut addrs: Vec<SocketAddr> = selected.and_then(direct_addr_of_hint).into_iter().collect();
    if let Some(known) = known {
        if relay.is_none() {
            relay = known.hints.iter().find_map(relay_url_of_hint);
        }
        let known_ips: Vec<SocketAddr> = known.hints.iter().filter_map(direct_addr_of_hint).collect();
        let ordered = known_ips
            .iter()
            .filter(|a| a.is_ipv4())
            .chain(known_ips.iter().filter(|a| a.is_ipv6()));
        for addr in ordered {
            if addrs.len() >= MAX_ADDRS {
                break;
            }
            if !addrs.contains(addr) {
                addrs.push(*addr);
            }
        }
    }
    RosterEntry {
        id: *peer.as_bytes(),
        relay,
        addrs,
        seen_unix: now_unix,
    }
}

impl PeersCache {
    /// Reads the cache from disk; never fails. `path = None` or no file — an empty cache silently;
    /// a read/parse error, a foreign version or a foreign network — an empty cache and a log line.
    /// Drops records older than `TTL_SECS`, keeps the `MAX_PEERS` freshest.
    pub fn load(path: Option<&Path>, network_id: &str, now_unix: u64) -> PeersCache {
        let mut peers = match path {
            Some(path) => read_valid(path, network_id).unwrap_or_else(|e| {
                say("cache", format!("cache {} not used: {e}", path.display()));
                Vec::new()
            }),
            None => Vec::new(),
        };
        peers.retain(|e| e.seen_unix.saturating_add(TTL_SECS) >= now_unix);
        peers.sort_by_key(|e| std::cmp::Reverse(e.seen_unix));
        peers.truncate(MAX_PEERS);
        PeersCache {
            path: path.map(Path::to_path_buf),
            network_id: network_id.to_string(),
            peers,
        }
    }

    /// Addresses of all cache members, from fresh to old; a corrupt record (a bad relay URL)
    /// is skipped silently.
    pub fn addrs(&self) -> Vec<PeerAddr> {
        self.peers.iter().filter_map(|e| entry_to_addr(e).ok()).collect()
    }

    /// Map "member id → its `seen_unix`" for all cache records; a record with an id that
    /// `PeerId::from_bytes` cannot parse is skipped silently (like `addrs`).
    pub fn seen(&self) -> HashMap<PeerId, u64> {
        self.peers
            .iter()
            .filter_map(|e| PeerId::from_bytes(&e.id).ok().map(|id| (id, e.seen_unix)))
            .collect()
    }

    /// Number of records in the cache.
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    /// The cache is empty.
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Inserts or replaces the record with the same `id`. Returns `true` if this is a new member,
    /// the `relay` or the set of addresses changed, or the previous confirmation is older than `REFRESH_SECS` — in
    /// these cases the record is updated and must be written to disk. Returns `false` without
    /// changing the cache if nothing changed (the record is younger than `REFRESH_SECS`), or if
    /// `entry` has no `relay` and no addresses — the cache does not take such a record at all.
    pub fn remember(&mut self, mut entry: RosterEntry, now_unix: u64) -> bool {
        if entry.relay.is_none() && entry.addrs.is_empty() {
            return false;
        }
        match self.peers.iter().position(|e| e.id == entry.id) {
            Some(idx) => {
                let prev = &self.peers[idx];
                let changed = prev.relay != entry.relay || prev.addrs != entry.addrs;
                let stale = prev.seen_unix.saturating_add(REFRESH_SECS) < now_unix;
                if !changed && !stale {
                    return false;
                }
                entry.seen_unix = now_unix;
                self.peers[idx] = entry;
            }
            None => {
                entry.seen_unix = now_unix;
                self.peers.push(entry);
            }
        }
        self.peers.sort_by_key(|e| std::cmp::Reverse(e.seen_unix));
        self.peers.retain(|e| e.seen_unix.saturating_add(TTL_SECS) >= now_unix);
        self.peers.truncate(MAX_PEERS);
        true
    }

    /// Refreshes `seen_unix` of an already known record without touching addresses. `true` if the record exists and
    /// was older than `REFRESH_SECS`; otherwise `false` without changing the cache.
    pub fn touch(&mut self, peer: PeerId, now_unix: u64) -> bool {
        let id = *peer.as_bytes();
        let Some(idx) = self.peers.iter().position(|e| e.id == id) else {
            return false;
        };
        if self.peers[idx].seen_unix.saturating_add(REFRESH_SECS) >= now_unix {
            return false;
        }
        self.peers[idx].seen_unix = now_unix;
        self.peers.sort_by_key(|e| std::cmp::Reverse(e.seen_unix));
        true
    }

    /// Removes from the cache the records of the listed members. `true` if anything was removed (the cache needs
    /// to be written to disk); it does not touch the disk itself.
    pub fn forget(&mut self, gone: &HashSet<PeerId>) -> bool {
        let before = self.peers.len();
        self.peers
            .retain(|e| !PeerId::from_bytes(&e.id).is_ok_and(|id| gone.contains(&id)));
        self.peers.len() != before
    }

    /// Writes the cache to disk atomically with `0600` permissions (`config_file::write_tmp_and_rename`).
    /// A disabled cache (`path = None`) — `Ok(())` without writing.
    pub fn save(&self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let file = CacheFile {
            version: PEERS_CACHE_FORMAT_VERSION,
            network_id: self.network_id.clone(),
            peers: self.peers.clone(),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("cache {} not serialized: {e}", path.display()))?;
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let tmp = dir.join(format!("{PEERS_CACHE_FILE_NAME}.tmp-{}", std::process::id()));
        write_tmp_and_rename(&tmp, path, &text)
            .map_err(|e| format!("cache {} not written: {e}", path.display()))
    }
}

#[cfg(test)]
#[path = "peers_cache_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "peers_cache_seen_tests.rs"]
mod seen_tests;

#[cfg(test)]
#[path = "peers_cache_forget_tests.rs"]
mod forget_tests;

#[cfg(test)]
#[path = "peers_cache_golden_tests.rs"]
mod golden_tests;

#[cfg(test)]
#[path = "peers_cache_hint_tests.rs"]
mod hint_tests;
