//! The one-time transfer of `docs-store/` into the files of its own replicas. Reads through an external reader
//! (`LegacyRead`), knows nothing about the old store format itself; the mark in the replica file makes a repeated start fast.

use super::entry::{seal, WireEntry, MAX_KEY_BYTES};
use super::file::{self, Bak, FileError, ImportMark, Loaded, MarkSource};
use super::ids::{domains_id, net_id};
use super::state::State;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::{NodeSecret, PeerId};
use crate::transport::net::replica::{ReplicaEntry, ReplicaId, MAX_VALUE_BYTES};
use crate::transport::net::stream::BoxFuture;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The entries of both replicas from `docs-store/`: all authors, tombstones with an empty value.
#[derive(Clone, Debug, Default)]
pub struct Legacy {
    pub net: Vec<ReplicaEntry>,
    pub domains: Vec<ReplicaEntry>,
}

/// The `docs-store/` reader: the whole directory, the network seed, its own id → entries. No network.
pub type LegacyRead =
    Arc<dyn Fn(PathBuf, [u8; 32], PeerId) -> BoxFuture<'static, NetResult<Legacy>> + Send + Sync>;

#[derive(Debug, PartialEq, Eq)]
enum Need {
    Marked,
    Absent,
    Read(PathBuf),
}

/// What to do with a replica: the mark exists — nothing; no `docs-store/docs` directory — an empty mark; otherwise read.
fn need(sync_dir: Option<&Path>, mark: Option<&ImportMark>) -> Need {
    if mark.is_some() {
        return Need::Marked;
    }
    match sync_dir {
        Some(dir) if dir.join("docs").exists() => Need::Read(dir.to_path_buf()),
        _ => Need::Absent,
    }
}

fn fits(e: &ReplicaEntry) -> bool {
    e.key.len() <= MAX_KEY_BYTES && e.value.len() <= MAX_VALUE_BYTES
}

/// `"net"`: only "my" entries, re-signed with the original `ts_micros`; the second number is how many foreign and unusable ones were skipped.
fn own_entries(
    secret: &NodeSecret,
    id: &ReplicaId,
    all: Vec<ReplicaEntry>,
) -> (Vec<WireEntry>, u32) {
    let me = secret.peer_id();
    let total = all.len();
    let kept: Vec<WireEntry> = all
        .into_iter()
        .filter(|e| e.author == me && fits(e))
        .map(|e| seal(secret, id, &e.key, &e.value, e.ts_micros))
        .collect();
    (kept.clone(), (total - kept.len()) as u32)
}

/// `"domains"`: per key, the entry with the greatest `(ts_micros, author)` among all, re-signed with its own key.
fn adopt_latest(secret: &NodeSecret, id: &ReplicaId, all: Vec<ReplicaEntry>) -> Vec<WireEntry> {
    let mut best: std::collections::BTreeMap<Vec<u8>, ReplicaEntry> = Default::default();
    for e in all.into_iter().filter(fits) {
        let rank = |x: &ReplicaEntry| (x.ts_micros, *x.author.as_bytes());
        if best.get(&e.key).is_none_or(|old| rank(&e) > rank(old)) {
            best.insert(e.key.clone(), e);
        }
    }
    let seal_one = |e: ReplicaEntry| seal(secret, id, &e.key, &e.value, e.ts_micros);
    best.into_values().map(seal_one).collect()
}

/// The file of one replica, read before the import.
struct ImportSlot {
    name: &'static str,
    id: ReplicaId,
    path: PathBuf,
    mark: Option<ImportMark>,
    existing: Vec<WireEntry>,
    bak: Bak,
}

fn parts(loaded: Loaded) -> (Option<ImportMark>, Vec<WireEntry>) {
    match loaded {
        Loaded::Fresh => (None, vec![]),
        Loaded::Main(m, e) | Loaded::Backup(m, e) => (m, e),
    }
}

fn config_err(e: FileError) -> NetError {
    NetError::Config(e.to_string())
}

/// Reads a replica file. Corrupt together with its `.bak`: with `tolerant` (both replicas) it goes to quarantine ("no mark",
/// the import will repeat), otherwise — `Err`.
fn load_slot(
    dir: &Path,
    name: &'static str,
    id: ReplicaId,
    bak: Bak,
    tolerant: bool,
) -> NetResult<ImportSlot> {
    let path = file::path_for(dir, name, &id);
    let loaded = match file::load(&path, &id) {
        Err(FileError::Broken { main, .. }) if tolerant => {
            let moved = file::quarantine(&main).map_err(NetError::Io)?;
            tracing::warn!(
                "[sync] replica file {name} is damaged, moved to {}; the import will repeat",
                moved.display()
            );
            Loaded::Fresh
        }
        other => other.map_err(config_err)?,
    };
    let (mark, existing) = parts(loaded);
    Ok(ImportSlot {
        name,
        id,
        path,
        mark,
        existing,
        bak,
    })
}

/// A free name for the directory being moved aside: `.old` → `.old-<secs>` → `.old-<secs>-<n>`, n = 2..=99.
fn aside_name(sync_dir: &Path, now_secs: u64) -> Option<PathBuf> {
    let first = file::suffixed(sync_dir, ".old");
    let stamped = file::suffixed(sync_dir, &format!(".old-{now_secs}"));
    let counted = (2..=99).map(|n| file::suffixed(sync_dir, &format!(".old-{now_secs}-{n}")));
    [first, stamped]
        .into_iter()
        .chain(counted)
        .find(|p| !p.exists())
}

/// Moves `docs-store/` aside to the first free name next to it and returns the new path.
fn set_aside(sync_dir: &Path, now_secs: u64) -> NetResult<PathBuf> {
    let hint = "rename it by hand — the domain list will start empty";
    let Some(to) = aside_name(sync_dir, now_secs) else {
        return Err(NetError::Config(format!(
            "docs-store/ could not be moved aside to {}: no free name; {hint}",
            file::suffixed(sync_dir, ".old").display()
        )));
    };
    std::fs::rename(sync_dir, &to).map_err(|e| {
        NetError::Config(format!(
            "docs-store/ could not be moved aside to {}: {e}; {hint}",
            to.display()
        ))
    })?;
    Ok(to)
}

fn now_secs() -> u64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    since.map(|d| d.as_secs()).unwrap_or(0)
}

/// There is no reader — `docs-store/` is moved aside (once per directory), no entries.
fn set_aside_logged(sync_dir: &Path) -> NetResult<Option<Legacy>> {
    let to = set_aside(sync_dir, now_secs())?;
    tracing::warn!(
        "[sync] old-version docs-store/ was not migrated — moved aside: {} → {}; the domain list will start empty",
        sync_dir.display(),
        to.display()
    );
    Ok(None)
}

async fn fetch(
    sync_dir: &Path,
    secret: &NodeSecret,
    seed: &[u8; 32],
    legacy: Option<&LegacyRead>,
    needs: &[Need],
) -> NetResult<Option<Legacy>> {
    if !needs.iter().any(|n| matches!(n, Need::Read(_))) {
        return Ok(None);
    }
    let Some(read) = legacy else {
        return set_aside_logged(sync_dir);
    };
    let got = read(sync_dir.to_path_buf(), *seed, secret.peer_id()).await;
    got.map(Some)
        .map_err(|e| NetError::Config(format!("docs-store/ was not read: {e}")))
}

fn now_micros() -> u64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    since.map(|d| d.as_micros() as u64).unwrap_or(0)
}

/// The entries to import and the skipped count; without `Legacy` — empty.
fn incoming(
    slot: &ImportSlot,
    secret: &NodeSecret,
    legacy: Option<&Legacy>,
) -> (Vec<WireEntry>, u32) {
    let Some(l) = legacy else { return (vec![], 0) };
    if slot.name == "net" {
        return own_entries(secret, &slot.id, l.net.clone());
    }
    (adopt_latest(secret, &slot.id, l.domains.clone()), 0)
}

/// Merges with what is already in the file and writes the file together with the mark. Returns the transferred entries.
fn write_slot(
    slot: ImportSlot,
    secret: &NodeSecret,
    legacy: Option<&Legacy>,
) -> NetResult<(Vec<WireEntry>, u32)> {
    let (fresh, skipped) = incoming(&slot, secret, legacy);
    let (mut state, _) = State::from_entries(slot.id, secret.peer_id(), slot.existing);
    fresh.iter().cloned().for_each(|w| {
        state.merge(w);
    });
    let source = if legacy.is_some() {
        MarkSource::DocsStore
    } else {
        MarkSource::Absent
    };
    let mark = ImportMark {
        at_micros: now_micros(),
        source,
        entries: fresh.len() as u32,
    };
    let bytes = file::encode(&slot.id, Some(&mark), &state.entries());
    file::save(&slot.path, &bytes, slot.bak).map_err(NetError::Io)?;
    Ok((fresh, skipped))
}

fn log_summary(domains: &[WireEntry], net: &[WireEntry], skipped: u32) {
    let list = |tomb: bool| {
        domains
            .iter()
            .filter(|w| w.key.starts_with(b"list/") && w.value.is_empty() == tomb)
            .count()
    };
    let (live, dead) = (list(false), list(true));
    tracing::info!(
        "[sync] migrated from docs-store: domains {live} (tombstones {dead}), the network's own entries {}, foreign skipped {skipped}",
        net.len()
    );
    if domains.is_empty() && net.is_empty() {
        tracing::warn!("[sync] docs-store/ exists, but there is nothing to migrate: no domains or the network's own entries found");
    }
}

/// The `docs-store/` import into the `"net"` and `"domains"` replica files. `None` (memory) does nothing.
/// There is no reader — `docs-store/` is moved aside, marks are `Absent`. A reader failure is `Err`, the replica files and marks are not written.
pub(crate) async fn import_docs_store(
    sync_dir: Option<&Path>,
    secret: &NodeSecret,
    network_seed: &[u8; 32],
    legacy: Option<&LegacyRead>,
) -> NetResult<()> {
    let Some(dir) = sync_dir.and_then(file::replica_dir) else {
        return Ok(());
    };
    let net = load_slot(&dir, "net", net_id(network_seed), Bak::Keep, true)?;
    let domains = load_slot(&dir, "domains", domains_id(), Bak::Mirror, true)?;
    let needs = [
        need(sync_dir, net.mark.as_ref()),
        need(sync_dir, domains.mark.as_ref()),
    ];
    let data = fetch(
        sync_dir.unwrap_or(Path::new("")),
        secret,
        network_seed,
        legacy,
        &needs,
    )
    .await?;
    let [need_net, need_domains] = needs;
    let (mut net_in, mut domains_in, mut skipped) = (vec![], vec![], 0);
    if need_net != Need::Marked {
        (net_in, skipped) = write_slot(net, secret, data.as_ref())?;
    }
    if need_domains != Need::Marked {
        (domains_in, _) = write_slot(domains, secret, data.as_ref())?;
    }
    if data.is_some() {
        log_summary(&domains_in, &net_in, skipped);
    }
    Ok(())
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "import_aside_tests.rs"]
mod aside_tests;
