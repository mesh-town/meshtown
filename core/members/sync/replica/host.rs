//! The host of the node's two replicas: reads their files (after the `docs-store/` handling) and hands them out by name.

use super::entry::WireEntry;
use super::file::{self, FileError, ImportMark, Loaded};
use super::ids::{domains_id, net_id};
use super::import;
use super::replica::MeshReplica;
use super::saver::{BakPolicy, Saver};
use super::state::State;
use crate::members::sync::flood::admit::admission_key;
use crate::members::sync::flood::FloodSync;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::{NodeSecret, PeerId};
use crate::transport::net::replica::ReplicaId;
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::WeakStack;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};

/// What the replicas get from the node after creation: the stack for dials, flooding for the broadcast topic.
pub(super) struct Wiring {
    pub stack: OnceLock<WeakStack>,
    pub flood: OnceLock<Weak<FloodSync>>,
    /// The admission key for `Hello`/`Welcome` from the network seed.
    pub admission_key: [u8; 32],
    /// The label of our own node in the handshake.
    pub me: PeerId,
}

pub(crate) struct ReplicaHost {
    net: Arc<MeshReplica>,
    domains: Arc<MeshReplica>,
    pub(super) wiring: Arc<Wiring>,
}

/// A replica brought up from a file.
struct Opened {
    state: State,
    mark: Option<ImportMark>,
    policy: BakPolicy,
}

/// Quarantines the main file and `.bak` (whichever exist): otherwise a corrupt `.bak` would stop the next start from being `Fresh`.
fn quarantine_both(main: &Path) -> std::io::Result<PathBuf> {
    let mut bak = main.file_name().unwrap_or_default().to_os_string();
    bak.push(".bak");
    let bak = main.with_file_name(bak);
    if bak.exists() {
        file::quarantine(&bak)?;
    }
    if main.exists() {
        return file::quarantine(main);
    }
    Ok(main.to_path_buf())
}

/// Moves a broken replica file (and its `.bak`) to quarantine and logs it.
fn set_aside_broken(main: &Path, name: &str, mirror: bool) -> NetResult<()> {
    let moved = quarantine_both(main).map_err(NetError::Io)?;
    match mirror {
        true => tracing::warn!("[sync] the domain list for the availability check is lost, starting with an empty one: the file was moved to {}", moved.display()),
        false => tracing::error!("[sync] replica file {name} is damaged, moved to {}; starting without it", moved.display()),
    }
    Ok(())
}

/// Loads the file; a broken one goes to quarantine and counts as absent.
fn load_replica_file(path: &Path, id: &ReplicaId, name: &str, mirror: bool) -> NetResult<Loaded> {
    match file::load(path, id) {
        Err(FileError::Broken { main, .. }) => {
            set_aside_broken(&main, name, mirror)?;
            Ok(Loaded::Fresh)
        }
        other => other.map_err(|e| NetError::Config(format!("replica {name}: {e}"))),
    }
}

/// Splits a loaded file into mark, entries and whether the main file was intact.
fn unpack_loaded(
    loaded: Loaded,
    name: &str,
    path: &Path,
) -> (Option<ImportMark>, Vec<WireEntry>, bool) {
    match loaded {
        Loaded::Fresh => (None, vec![], false),
        Loaded::Main(m, e) => (m, e, true),
        Loaded::Backup(m, e) => {
            tracing::warn!(
                "[sync] replica file {name} is damaged, using .bak: {}",
                path.display()
            );
            (m, e, false)
        }
    }
}

/// Logs the entries dropped for a bad signature while reading.
fn warn_rejected(name: &str, rejected: u32) {
    if rejected == 0 {
        return;
    }
    tracing::warn!("[sync] replica {name}: entries with a bad signature were dropped while reading the file: {rejected}");
}

/// The `.bak` policy: the domain list mirrors, the rest keeps the first intact main file.
fn bak_policy(mirror: bool, main_ok: bool) -> BakPolicy {
    if mirror {
        return BakPolicy::Mirror;
    }
    BakPolicy::FirstSave { main_ok }
}

/// Reads a replica file; on corruption both replicas go to quarantine and start empty
/// (`"net"` — `error!`, the domain list for the availability check — `warn!`, it can be lost).
fn read_file(path: &Path, id: ReplicaId, secret: &NodeSecret, name: &str) -> NetResult<Opened> {
    let mirror = name == REPLICA_DOMAINS;
    let loaded = load_replica_file(path, &id, name, mirror)?;
    let (mark, entries, main_ok) = unpack_loaded(loaded, name, path);
    let (state, rejected) = State::from_entries(id, secret.peer_id(), entries);
    warn_rejected(name, rejected);
    Ok(Opened {
        state,
        mark,
        policy: bak_policy(mirror, main_ok),
    })
}

fn open_one(
    ctx: &SyncCtx,
    wiring: &Arc<Wiring>,
    name: &'static str,
    id: ReplicaId,
    dir: Option<&PathBuf>,
) -> NetResult<Arc<MeshReplica>> {
    let path = dir.map(|d| file::path_for(d, name, &id));
    let opened = match &path {
        Some(p) => read_file(p, id, &ctx.secret, name)?,
        None => Opened {
            state: State::new(id, ctx.secret.peer_id()),
            mark: None,
            policy: BakPolicy::Mirror,
        },
    };
    let saver = Saver::new(path, id, opened.mark, opened.policy);
    Ok(MeshReplica::new(
        name,
        ctx.secret.clone(),
        opened.state,
        saver,
        wiring.clone(),
    ))
}

impl ReplicaHost {
    /// The `docs-store/` handling (if needed) → reading the files → both replicas.
    pub(crate) async fn load(ctx: &SyncCtx) -> NetResult<Arc<ReplicaHost>> {
        import::import_docs_store(ctx.store_dir.as_deref(), &ctx.secret, &ctx.network_seed).await?;
        let dir = ctx.store_dir.as_deref().and_then(file::replica_dir);
        let wiring = Arc::new(Wiring {
            stack: OnceLock::new(),
            flood: OnceLock::new(),
            admission_key: admission_key(&ctx.network_seed),
            me: ctx.secret.peer_id(),
        });
        Ok(Arc::new(ReplicaHost {
            net: open_one(
                ctx,
                &wiring,
                REPLICA_NET,
                net_id(&ctx.network_seed),
                dir.as_ref(),
            )?,
            domains: open_one(ctx, &wiring, REPLICA_DOMAINS, domains_id(), dir.as_ref())?,
            wiring,
        }))
    }

    /// A replica by name; any other name is `Config`.
    pub(crate) fn open(&self, name: &str) -> NetResult<Arc<MeshReplica>> {
        match name {
            REPLICA_NET => Ok(self.net.clone()),
            REPLICA_DOMAINS => Ok(self.domains.clone()),
            other => Err(NetError::Config(format!("no replica \"{other}\""))),
        }
    }

    /// A replica that can be matched by such an id; `"domains"` never leaves for the network.
    pub(crate) fn syncing(&self, id: &[u8; 32]) -> Option<Arc<MeshReplica>> {
        (self.net.id.as_bytes() == id).then(|| self.net.clone())
    }

    pub(crate) fn attach_stack(&self, stack: WeakStack) {
        let _ = self.wiring.stack.set(stack);
    }

    pub(crate) fn attach_flood(&self, flood: Weak<FloodSync>) {
        let _ = self.wiring.flood.set(flood);
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
