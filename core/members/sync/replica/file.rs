//! The replica file: the format, reading with `.bak`, atomic byte writing, quarantining a corrupt file.
//! Entry signatures are not checked here: `load` returns raw `WireEntry`, the check is done by `State::from_entries`.

use super::entry::WireEntry;
use crate::transport::net::replica::ReplicaId;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"MVRP";
const VERSION: u16 = 1;
const HEADER: usize = 4 + 2 + 32;

/// Where the file contents came from during the one-time import.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MarkSource {
    Absent,
    DocsStore,
}

/// The "import from `docs-store/` done" mark; stored in the same file as the data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ImportMark {
    pub at_micros: u64,
    pub source: MarkSource,
    pub entries: u32,
}

#[derive(Serialize, Deserialize)]
struct FileBody {
    replica: [u8; 32],
    import: Option<ImportMark>,
    entries: Vec<WireEntry>,
}

/// Why the file was not read.
#[derive(Debug)]
pub(crate) enum FileError {
    BadMagic,
    BadVersion(u16),
    BadChecksum,
    OtherReplica,
    Body(String),
    Io(io::Error),
    /// The main file and `.bak` are unusable (or there is no main file and the `.bak` is bad).
    Broken {
        main: PathBuf,
        bak: PathBuf,
        reason: String,
    },
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::BadMagic => write!(f, "not a replica file (magic)"),
            FileError::BadVersion(v) => write!(f, "format version {v} is not supported"),
            FileError::BadChecksum => write!(f, "body checksum mismatch"),
            FileError::OtherReplica => write!(f, "file of another replica"),
            FileError::Body(e) => write!(f, "file body could not be parsed: {e}"),
            FileError::Io(e) => write!(f, "read error: {e}"),
            FileError::Broken { main, bak, reason } => write!(
                f,
                "replica files are damaged: {} and {} ({reason})",
                main.display(),
                bak.display()
            ),
        }
    }
}

impl std::error::Error for FileError {}

/// What was found on disk.
#[derive(Debug)]
pub(crate) enum Loaded {
    Fresh,
    Main(Option<ImportMark>, Vec<WireEntry>),
    Backup(Option<ImportMark>, Vec<WireEntry>),
}

/// What to do with `.bak` on save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bak {
    Keep,
    CopyPrevious,
    Mirror,
}

/// The replica file directory: next to the sync directory (`docs-store/`).
pub(crate) fn replica_dir(sync_dir: &Path) -> Option<PathBuf> {
    Some(sync_dir.parent()?.join("replica"))
}

/// The replica file path: `domains.bin` or `<name>-<10 hex id>.bin`.
pub(crate) fn path_for(dir: &Path, name: &str, id: &ReplicaId) -> PathBuf {
    if name == "domains" {
        return dir.join("domains.bin");
    }
    dir.join(format!("{name}-{}.bin", id.fmt_short()))
}

fn bak_path(path: &Path) -> PathBuf {
    suffixed(path, ".bak")
}

pub(super) fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// The file: magic, version, blake3 of the body, the postcard body.
pub(crate) fn encode(
    id: &ReplicaId,
    import: Option<&ImportMark>,
    entries: &[WireEntry],
) -> Vec<u8> {
    let body = FileBody {
        replica: *id.as_bytes(),
        import: import.copied(),
        entries: entries.to_vec(),
    };
    let body = postcard::to_allocvec(&body).expect("postcard writes to a Vec");
    let mut out = Vec::with_capacity(HEADER + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_be_bytes());
    out.extend_from_slice(blake3::hash(&body).as_bytes());
    out.extend_from_slice(&body);
    out
}

pub(crate) fn decode(
    id: &ReplicaId,
    bytes: &[u8],
) -> Result<(Option<ImportMark>, Vec<WireEntry>), FileError> {
    if bytes.len() < HEADER || &bytes[..4] != MAGIC {
        return Err(FileError::BadMagic);
    }
    let version = u16::from_be_bytes([bytes[4], bytes[5]]);
    if version != VERSION {
        return Err(FileError::BadVersion(version));
    }
    let body = &bytes[HEADER..];
    if blake3::hash(body).as_bytes() != &bytes[6..HEADER] {
        return Err(FileError::BadChecksum);
    }
    let body: FileBody = postcard::from_bytes(body).map_err(|e| FileError::Body(e.to_string()))?;
    if body.replica != *id.as_bytes() {
        return Err(FileError::OtherReplica);
    }
    Ok((body.import, body.entries))
}

type Content = (Option<ImportMark>, Vec<WireEntry>);

fn read_one(path: &Path, id: &ReplicaId) -> Result<Option<Content>, FileError> {
    match std::fs::read(path) {
        Ok(bytes) => decode(id, &bytes).map(Some),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(FileError::Io(e)),
    }
}

/// The main file → `.bak` → `Broken`. If there is neither — `Fresh`.
pub(crate) fn load(path: &Path, id: &ReplicaId) -> Result<Loaded, FileError> {
    let main = read_one(path, id);
    if let Ok(Some((m, e))) = main {
        return Ok(Loaded::Main(m, e));
    }
    let bak = bak_path(path);
    match (read_one(&bak, id), main) {
        (Ok(Some((m, e))), _) => Ok(Loaded::Backup(m, e)),
        (Ok(None), Ok(None)) => Ok(Loaded::Fresh),
        (bak_res, main_res) => {
            let reason = [main_res.err(), bak_res.err()]
                .into_iter()
                .flatten()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            Err(FileError::Broken {
                main: path.to_path_buf(),
                bak,
                reason,
            })
        }
    }
}

/// Atomically write bytes; `.bak` per `bak`.
pub(crate) fn save(path: &Path, bytes: &[u8], bak: Bak) -> io::Result<()> {
    let previous = match bak {
        Bak::CopyPrevious => std::fs::read(path).ok(),
        _ => None,
    };
    if let Some(prev) = previous {
        write_atomic(&bak_path(path), &prev)?;
    }
    write_atomic(path, bytes)?;
    if bak == Bak::Mirror {
        write_atomic(&bak_path(path), bytes)?;
    }
    Ok(())
}

/// Move a corrupt file out of the way: `<name>.broken-<unix seconds>`.
pub(crate) fn quarantine(path: &Path) -> io::Result<PathBuf> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let to = suffixed(path, &format!(".broken-{secs}"));
    std::fs::rename(path, &to)?;
    Ok(to)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    private_dir(dir)?;
    let tmp = suffixed(path, ".tmp");
    let mut file = open_private(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path)?;
    sync_dir(dir)
}

fn open_private(path: &Path) -> io::Result<std::fs::File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(path)
}

fn private_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    Ok(())
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    std::fs::File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
