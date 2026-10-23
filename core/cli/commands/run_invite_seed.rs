//! What a network added from the command line starts with: the peer addresses from the invite (as the
//! neighbour cache beside the slot file) and the site list (as the slot's replica file).
//! Both files are the ones the node reads when it starts the slot.

use std::io;
use std::path::Path;

use crate::members::sync::replica::entry::{seal, WireEntry};
use crate::members::sync::replica::file::{self as replica_file, Bak};
use crate::members::sync::replica::ids::domains_id;
use crate::proxy::domains::{ENTRY_VALUE, LIST_PREFIX};
use crate::transport::net::{NodeSecret, PeerAddr};
use crate::transport::peers_cache::{self, peers_cache_path_beside, PeersCache};

/// Writes the invite's addresses into the neighbour cache beside `slot_file`. No addresses, no file.
pub(crate) fn seed_peers_cache(
    slot_file: &Path,
    network_id_hex: &str,
    peers: &[PeerAddr],
    now: u64,
) -> Result<(), String> {
    if peers.is_empty() {
        return Ok(());
    }
    let path = peers_cache_path_beside(slot_file);
    let mut cache = PeersCache::load(path.as_deref(), network_id_hex, now);
    for addr in peers {
        let entry = peers_cache::entry_for(Some(addr), addr.id, None, now);
        cache.remember(entry, now);
    }
    cache.save()
}

/// Writes the site list into `replica/domains.bin` beside `slot_file`, signed by the slot's node key.
/// An empty list writes nothing.
pub(crate) fn seed_domains(
    slot_file: &Path,
    secret: &NodeSecret,
    domains: &[String],
    now_micros: u64,
) -> io::Result<()> {
    let Some(dir) = slot_file.parent() else {
        return Ok(());
    };
    if domains.is_empty() {
        return Ok(());
    }
    let id = domains_id();
    let entries: Vec<WireEntry> = domains
        .iter()
        .map(|d| {
            let key = format!("{LIST_PREFIX}{d}/");
            seal(secret, &id, key.as_bytes(), ENTRY_VALUE, now_micros)
        })
        .collect();
    let folder = dir.join("replica");
    std::fs::create_dir_all(&folder)?;
    let bytes = replica_file::encode(&id, None, &entries);
    let path = replica_file::path_for(&folder, "domains", &id);
    replica_file::save(&path, &bytes, Bak::Keep)
}
