//! The one-time handling of `docs-store/` at start: the directory is moved aside and the files of the node's own replicas
//! get an `Absent` mark; the mark makes a repeated start fast.

use super::entry::WireEntry;
use super::file::{self, Bak, FileError, ImportMark, Loaded, MarkSource};
use super::ids::{domains_id, net_id};
use super::state::State;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::NodeSecret;
use crate::transport::net::replica::ReplicaId;
use std::path::{Path, PathBuf};

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

/// The file of one replica, read before the import.
struct ImportSlot {
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

/// `docs-store/` is set aside (once per directory) when some replica has no mark yet and the directory holds data.
fn set_aside_logged(sync_dir: &Path) -> NetResult<()> {
    let to = set_aside(sync_dir, now_secs())?;
    tracing::warn!(
        "[sync] old-version docs-store/ was not migrated — moved aside: {} → {}; the domain list will start empty",
        sync_dir.display(),
        to.display()
    );
    Ok(())
}

fn now_micros() -> u64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    since.map(|d| d.as_micros() as u64).unwrap_or(0)
}

/// Writes the replica file with the `Absent` mark, keeping the entries already in it.
fn write_slot(slot: ImportSlot, secret: &NodeSecret) -> NetResult<()> {
    let (state, _) = State::from_entries(slot.id, secret.peer_id(), slot.existing);
    let mark = ImportMark {
        at_micros: now_micros(),
        source: MarkSource::Absent,
        entries: 0,
    };
    let bytes = file::encode(&slot.id, Some(&mark), &state.entries());
    file::save(&slot.path, &bytes, slot.bak).map_err(NetError::Io)
}

/// The `docs-store/` handling for the `"net"` and `"domains"` replica files. `None` (memory) does nothing.
/// `docs-store/` is moved aside, the marks of both replicas are `Absent`; a replica with a mark is left alone.
pub(crate) async fn import_docs_store(
    sync_dir: Option<&Path>,
    secret: &NodeSecret,
    network_seed: &[u8; 32],
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
    if let Some(Need::Read(aside)) = needs.iter().find(|n| matches!(n, Need::Read(_))) {
        set_aside_logged(aside)?;
    }
    let [need_net, need_domains] = needs;
    if need_net != Need::Marked {
        write_slot(net, secret)?;
    }
    if need_domains != Need::Marked {
        write_slot(domains, secret)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "import_aside_tests.rs"]
mod aside_tests;
