//! Tests for `import.rs`: the `docs-store/` transfer with a fake reader.

use super::*;
use crate::sync::replica::entry::check;
use crate::sync::replica::file::{self, Loaded};
use crate::sync::replica::ids::{domains_id, net_id};
use crate::sync::replica::state::State;
use bytes::Bytes;
use std::sync::atomic::{AtomicUsize, Ordering};
use portable_atomic::AtomicU64;
use std::sync::Mutex;

pub(super) const SEED: [u8; 32] = [7; 32];

pub(super) fn temp_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-replica-import-{}-{}", std::process::id(), seq));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn secret() -> NodeSecret {
    NodeSecret::from_bytes(&[1; 32])
}

fn other() -> PeerId {
    NodeSecret::from_bytes(&[2; 32]).peer_id()
}

fn ent(author: PeerId, key: &str, value: &str, ts: u64) -> ReplicaEntry {
    ReplicaEntry { author, key: key.as_bytes().to_vec(), value: Bytes::from(value.to_owned()), ts_micros: ts }
}

fn mine(key: &str, value: &str, ts: u64) -> ReplicaEntry {
    ent(secret().peer_id(), key, value, ts)
}

/// A fake reader: a call counter and an answer that can be changed (`None` — a read error).
pub(super) struct Fake {
    calls: Arc<AtomicUsize>,
    answer: Arc<Mutex<Option<Legacy>>>,
}

impl Fake {
    pub(super) fn new(answer: Option<Legacy>) -> Fake {
        Fake { calls: Arc::new(AtomicUsize::new(0)), answer: Arc::new(Mutex::new(answer)) }
    }

    pub(super) fn reader(&self) -> LegacyRead {
        let (calls, answer) = (self.calls.clone(), self.answer.clone());
        Arc::new(move |_dir, _seed, _me| {
            calls.fetch_add(1, Ordering::SeqCst);
            let got = answer.lock().unwrap().clone();
            Box::pin(async move { got.ok_or_else(|| NetError::Config("docs.redb is busy".into())) })
        })
    }

    pub(super) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
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
    let id = if name == "net" { net_id(&SEED) } else { domains_id() };
    file::path_for(&file::replica_dir(sync_dir).unwrap(), name, &id)
}

pub(super) fn state_of(sync_dir: &Path, name: &str) -> (Option<ImportMark>, State) {
    let id = if name == "net" { net_id(&SEED) } else { domains_id() };
    let (mark, entries) = match file::load(&path_of(sync_dir, name), &id).unwrap() {
        Loaded::Fresh => (None, vec![]),
        Loaded::Main(m, e) | Loaded::Backup(m, e) => (m, e),
    };
    let (state, rejected) = State::from_entries(id, secret().peer_id(), entries);
    assert_eq!(rejected, 0, "signatures must pass");
    (mark, state)
}

pub(super) async fn run_with(sync_dir: &Path, fake: &Fake) -> NetResult<()> {
    import_docs_store(Some(sync_dir), &secret(), &SEED, Some(&fake.reader())).await
}

#[tokio::test]
async fn no_docs_store_marks_absent() {
    let dir = node(false);
    let fake = Fake::new(Some(Legacy::default()));
    run_with(&dir, &fake).await.unwrap();
    assert_eq!(fake.calls(), 0);
    for name in ["net", "domains"] {
        let (mark, state) = state_of(&dir, name);
        assert_eq!(mark.unwrap().source, MarkSource::Absent);
        assert!(state.scan(b"").is_empty());
    }
}

#[tokio::test]
async fn imports_domains_and_own_net() {
    let dir = node(true);
    let legacy = Legacy {
        domains: vec![
            mine("list/a", "1", 10),
            mine("list/b", "2", 20),
            mine("meta/own-list-from-shared/x", "done", 30),
        ],
        net: vec![mine("invite/1", "tok", 40), mine("node/name/", "n1", 50)],
    };
    run_with(&dir, &Fake::new(Some(legacy.clone()))).await.unwrap();
    for (name, want) in [("domains", &legacy.domains), ("net", &legacy.net)] {
        let (mark, state) = state_of(&dir, name);
        assert_eq!(mark.unwrap().source, MarkSource::DocsStore);
        let got = state.scan(b"");
        assert_eq!(got.len(), want.len());
        for w in want {
            assert!(got.contains(w), "{name}: missing {:?}", w.key);
        }
        let id = if name == "net" { net_id(&SEED) } else { domains_id() };
        for w in state.entries() {
            assert!(check(&id, &w).is_ok());
        }
    }
}

#[tokio::test]
async fn foreign_entries_skipped() {
    let dir = node(true);
    let legacy = Legacy {
        domains: vec![],
        net: vec![mine("node/name/", "me", 5), ent(other(), "node/name/", "them", 6)],
    };
    run_with(&dir, &Fake::new(Some(legacy))).await.unwrap();
    let (_, state) = state_of(&dir, "net");
    let got = state.scan(b"");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].author, secret().peer_id());
}

#[tokio::test]
async fn tombstones_kept() {
    let dir = node(true);
    let legacy = Legacy { domains: vec![mine("list/c", "", 9)], net: vec![] };
    run_with(&dir, &Fake::new(Some(legacy))).await.unwrap();
    let (_, state) = state_of(&dir, "domains");
    assert!(state.scan(b"list/")[0].is_tombstone());
    assert!(state.latest_per_key(b"list/")[0].is_tombstone());
}

#[tokio::test]
async fn second_start_does_not_read() {
    let dir = node(true);
    let fake = Fake::new(Some(Legacy { domains: vec![mine("list/a", "1", 1)], net: vec![] }));
    run_with(&dir, &fake).await.unwrap();
    run_with(&dir, &fake).await.unwrap();
    assert_eq!(fake.calls(), 1);
}

#[tokio::test]
async fn reader_error_fails_start_without_marks() {
    let dir = node(true);
    let fake = Fake::new(None);
    let err = run_with(&dir, &fake).await.unwrap_err();
    assert!(err.to_string().contains("docs.redb"));
    assert!(!path_of(&dir, "net").exists());
    assert!(!path_of(&dir, "domains").exists());
    *fake.answer.lock().unwrap() = Some(Legacy { domains: vec![mine("list/a", "1", 1)], net: vec![] });
    run_with(&dir, &fake).await.unwrap();
    assert_eq!(state_of(&dir, "domains").1.scan(b"list/").len(), 1);
}

#[tokio::test]
async fn partial_import_resumes() {
    let dir = node(true);
    let first = Fake::new(Some(Legacy { domains: vec![mine("list/a", "1", 1)], net: vec![] }));
    run_with(&dir, &first).await.unwrap();
    std::fs::remove_file(path_of(&dir, "net")).unwrap();
    let before = std::fs::read(path_of(&dir, "domains")).unwrap();
    let second = Fake::new(Some(Legacy {
        domains: vec![mine("list/zzz", "new", 99)],
        net: vec![mine("node/name/", "n", 2)],
    }));
    run_with(&dir, &second).await.unwrap();
    assert_eq!(second.calls(), 1);
    assert_eq!(std::fs::read(path_of(&dir, "domains")).unwrap(), before);
    assert_eq!(state_of(&dir, "net").1.scan(b"").len(), 1);
}

#[tokio::test]
async fn marked_without_reader_ok() {
    // given: docs-store/ has already been migrated (the marks are set)
    let dir = node(true);
    run_with(&dir, &Fake::new(Some(Legacy { domains: vec![mine("list/a", "1", 5)], net: vec![] }))).await.unwrap();
    // when: a new build starts, there is no reader
    // then: no error
    import_docs_store(Some(&dir), &secret(), &SEED, None).await.unwrap();
}

#[tokio::test]
async fn domains_foreign_author_adopted() {
    let dir = node(true);
    let legacy = Legacy {
        domains: vec![mine("list/a", "old", 10), ent(other(), "list/a", "newer", 20)],
        net: vec![],
    };
    run_with(&dir, &Fake::new(Some(legacy))).await.unwrap();
    let (_, state) = state_of(&dir, "domains");
    let latest = state.latest_per_key(b"list/");
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].value, Bytes::from("newer"));
    assert_eq!(latest[0].author, secret().peer_id());
    assert_eq!(latest[0].ts_micros, 20);
}

#[tokio::test]
async fn broken_net_reimports() {
    let dir = node(true);
    let fake = Fake::new(Some(Legacy { domains: vec![], net: vec![mine("node/name/", "n", 2)] }));
    run_with(&dir, &fake).await.unwrap();
    std::fs::write(path_of(&dir, "net"), b"garbage").unwrap();
    run_with(&dir, &fake).await.unwrap();
    assert_eq!(fake.calls(), 2);
    assert_eq!(state_of(&dir, "net").1.scan(b"").len(), 1);
    let broken = std::fs::read_dir(path_of(&dir, "net").parent().unwrap())
        .unwrap()
        .any(|e| e.unwrap().file_name().to_string_lossy().contains(".broken-"));
    assert!(broken);
}

#[tokio::test]
async fn broken_domains_reimports() {
    let dir = node(true);
    let fake = Fake::new(Some(Legacy { domains: vec![mine("list/a", "1", 5)], net: vec![] }));
    run_with(&dir, &fake).await.unwrap();
    let path = path_of(&dir, "domains");
    std::fs::write(&path, b"garbage").unwrap();
    let mut bak = path.file_name().unwrap().to_os_string();
    bak.push(".bak");
    std::fs::write(path.with_file_name(bak), b"garbage").unwrap();
    run_with(&dir, &fake).await.unwrap();
    assert_eq!(fake.calls(), 2);
    assert_eq!(state_of(&dir, "domains").1.scan(b"").len(), 1);
}
