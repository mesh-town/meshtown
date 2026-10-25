//! Tests for `import.rs`: the `docs-store/` handling at start (marks, setting aside). Helpers shared with `import_aside_tests.rs`.

use super::*;
use crate::members::sync::replica::file::{self, Loaded};
use crate::members::sync::replica::ids::{domains_id, net_id};
use crate::members::sync::replica::state::State;
use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;

pub(super) const SEED: [u8; 32] = [7; 32];

pub(super) fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "mesh-replica-import-{}-{}",
        std::process::id(),
        seq
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn secret() -> NodeSecret {
    NodeSecret::from_bytes(&[1; 32])
}

/// A node in a temporary directory: `docs-store/` (with or without `docs/`) and `replica/` next to it.
pub(super) fn node(with_docs: bool) -> PathBuf {
    let sync_dir = temp_dir().join("docs-store");
    std::fs::create_dir_all(&sync_dir).unwrap();
    if with_docs {
        std::fs::create_dir_all(sync_dir.join("docs")).unwrap();
    }
    sync_dir
}

pub(super) fn path_of(sync_dir: &Path, name: &str) -> PathBuf {
    let id = if name == "net" {
        net_id(&SEED)
    } else {
        domains_id()
    };
    file::path_for(&file::replica_dir(sync_dir).unwrap(), name, &id)
}

pub(super) fn state_of(sync_dir: &Path, name: &str) -> (Option<ImportMark>, State) {
    let id = if name == "net" {
        net_id(&SEED)
    } else {
        domains_id()
    };
    let (mark, entries) = match file::load(&path_of(sync_dir, name), &id).unwrap() {
        Loaded::Fresh => (None, vec![]),
        Loaded::Main(m, e) | Loaded::Backup(m, e) => (m, e),
    };
    let (state, rejected) = State::from_entries(id, secret().peer_id(), entries);
    assert_eq!(rejected, 0, "signatures must pass");
    (mark, state)
}

pub(super) async fn run(sync_dir: &Path) -> NetResult<()> {
    import_docs_store(Some(sync_dir), &secret(), &SEED).await
}

#[tokio::test]
async fn no_docs_store_marks_absent() {
    let dir = node(false);
    run(&dir).await.unwrap();
    for name in ["net", "domains"] {
        let (mark, state) = state_of(&dir, name);
        assert_eq!(mark.unwrap().source, MarkSource::Absent);
        assert!(state.scan(b"").is_empty());
    }
}

#[tokio::test]
async fn broken_net_file_is_quarantined_and_marked_again() {
    // given: a start that set docs-store/ aside, then a damaged net file and a new docs-store/
    let dir = node(true);
    run(&dir).await.unwrap();
    std::fs::write(path_of(&dir, "net"), b"garbage").unwrap();
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    // when
    run(&dir).await.unwrap();
    // then: the file is readable again with an `Absent` mark, the damaged one is kept aside
    assert_eq!(state_of(&dir, "net").0.unwrap().source, MarkSource::Absent);
    let broken = std::fs::read_dir(path_of(&dir, "net").parent().unwrap())
        .unwrap()
        .any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".broken-")
        });
    assert!(broken);
}
