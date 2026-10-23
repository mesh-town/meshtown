//! Tests for `MeshReplica` on disk: write before `Ok`, restart, the import mark, `.bak`, a corrupt file.

use super::entry::{seal, WireEntry};
use super::file::{self, Bak, ImportMark, MarkSource};
use super::ids::{domains_id, net_id};
use super::sync_ctor;
use super::test_support::temp_dir;
use crate::members::sync::flood::test_support::NETWORK;
use crate::transport::net::contract::{must_finish, secret, SEED_A, SEED_B, STEP};
use crate::transport::net::replica::{Replica, ReplicaEntry};
use crate::transport::net::sync_backend::{SyncBackend, SyncCtx};
use crate::transport::net::NetResult;
use bytes::Bytes;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

fn ctx(base: &Path) -> SyncCtx {
    SyncCtx {
        secret: secret(SEED_A),
        network_seed: NETWORK,
        store_dir: Some(base.join("docs-store")),
    }
}

async fn try_start(base: &Path) -> NetResult<Arc<dyn SyncBackend>> {
    sync_ctor()(ctx(base)).await
}

async fn start(base: &Path) -> Arc<dyn SyncBackend> {
    try_start(base)
        .await
        .unwrap_or_else(|e| panic!("start failed: {e}"))
}

async fn open(sync: &Arc<dyn SyncBackend>, name: &str) -> Arc<dyn Replica> {
    must_finish(sync.open_replica(name))
        .await
        .expect("the replica opened")
}

fn domains_path(base: &Path) -> PathBuf {
    base.join("replica").join("domains.bin")
}

fn net_path(base: &Path) -> PathBuf {
    file::path_for(&base.join("replica"), "net", &net_id(&NETWORK))
}

fn bak_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

fn read_entries(
    path: &Path,
    id: &crate::transport::net::ReplicaId,
) -> (Option<ImportMark>, Vec<WireEntry>) {
    file::decode(id, &std::fs::read(path).expect("the file exists")).expect("the file is intact")
}

fn val(s: &str) -> Bytes {
    Bytes::copy_from_slice(s.as_bytes())
}

/// Waits up to `STEP` until the `"net"` file holds `n` entries.
async fn wait_net_file(base: &Path, n: usize) {
    let polling = async {
        loop {
            let ok = std::fs::read(net_path(base))
                .ok()
                .and_then(|b| file::decode(&net_id(&NETWORK), &b).ok());
            if ok.is_some_and(|(_, e)| e.len() == n) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(STEP, polling)
        .await
        .expect("the net file was not completed");
}

mod persist {
    use super::*;

    #[tokio::test]
    async fn local_write_on_disk_before_ok() {
        // given: "domains" on disk → when: set_own returned Ok → then: the file already contains the entry
        let base = temp_dir("disk");
        let d = open(&start(&base).await, "domains").await;
        must_finish(d.set_own(b"list/a", val("x"))).await.unwrap();
        let (_, entries) = read_entries(&domains_path(&base), &domains_id());
        assert!(entries
            .iter()
            .any(|w| w.key == b"list/a" && w.value == b"x"));
    }

    #[tokio::test]
    async fn flush_puts_a_delayed_write_on_disk() {
        // given: the "net" replica, which saves with a delay; a fresh own record
        let base = temp_dir("flush");
        let n = open(&start(&base).await, "net").await;
        must_finish(n.set_own(b"k/a", val("x"))).await.unwrap();
        // when: flush right away → then: the file already holds the entry
        must_finish(n.flush()).await.unwrap();
        let (_, entries) = read_entries(&net_path(&base), &net_id(&NETWORK));
        assert!(entries.iter().any(|w| w.key == b"k/a" && w.value == b"x"));
    }

    #[tokio::test]
    async fn reopen_keeps_entries_and_ts() {
        // given: in "net" an entry and a tombstone, the file is complete → when: a new sync over the directory → then: the same ReplicaEntry
        let base = temp_dir("reopen");
        let first = open(&start(&base).await, "net").await;
        must_finish(first.set_own(b"k1/", val("v1"))).await.unwrap();
        must_finish(first.set_own(b"k2/", val("v2"))).await.unwrap();
        must_finish(first.del_own(b"k2/")).await.unwrap();
        wait_net_file(&base, 2).await;
        let before: Vec<ReplicaEntry> = must_finish(first.scan(b"")).await.unwrap();
        let second = open(&start(&base).await, "net").await;
        assert_eq!(must_finish(second.scan(b"")).await.unwrap(), before);
        assert!(before.iter().any(|e| e.is_tombstone()));
    }

    #[tokio::test]
    async fn mark_survives_save() {
        // given: the "domains" file with an import mark → when: set_own → then: the mark is in place in the new file
        let base = temp_dir("mark");
        let mark = ImportMark {
            at_micros: 5,
            source: MarkSource::DocsStore,
            entries: 3,
        };
        std::fs::create_dir_all(base.join("replica")).unwrap();
        file::save(
            &domains_path(&base),
            &file::encode(&domains_id(), Some(&mark), &[]),
            Bak::Keep,
        )
        .unwrap();
        let d = open(&start(&base).await, "domains").await;
        must_finish(d.set_own(b"list/a", val("x"))).await.unwrap();
        let (got, entries) = read_entries(&domains_path(&base), &domains_id());
        assert_eq!(got, Some(mark));
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn bak_kept_when_started_from_bak() {
        // given: "net": the main file is corrupt, .bak is intact (with a mark) → when: start and write → then: .bak is unchanged
        let base = temp_dir("bak");
        let mark = ImportMark {
            at_micros: 5,
            source: MarkSource::Absent,
            entries: 0,
        };
        let old = seal(&secret(SEED_A), &net_id(&NETWORK), b"old/", b"1", 10);
        let good = file::encode(&net_id(&NETWORK), Some(&mark), &[old]);
        std::fs::create_dir_all(base.join("replica")).unwrap();
        std::fs::write(net_path(&base), b"garbage").unwrap();
        std::fs::write(bak_of(&net_path(&base)), &good).unwrap();
        let n = open(&start(&base).await, "net").await;
        must_finish(n.set_own(b"new/", val("2"))).await.unwrap();
        wait_net_file(&base, 2).await;
        assert_eq!(
            std::fs::read(bak_of(&net_path(&base))).unwrap(),
            good,
            ".bak was changed"
        );
    }

    #[tokio::test]
    async fn domains_mirror_survives_main_loss() {
        // given: two "domains" entries, the main file is truncated → when: a new start → then: both are in place from .bak
        let base = temp_dir("mirror");
        let d = open(&start(&base).await, "domains").await;
        must_finish(d.set_own(b"list/a", val("1"))).await.unwrap();
        must_finish(d.set_own(b"list/b", val("2"))).await.unwrap();
        let bytes = std::fs::read(domains_path(&base)).unwrap();
        std::fs::write(domains_path(&base), &bytes[..10]).unwrap();
        let again = open(&start(&base).await, "domains").await;
        assert_eq!(
            must_finish(again.latest_per_key(b"list/"))
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn domains_other_author_accepted() {
        // given: domains.bin has an entry of another key (the secret changed) → then: latest_per_key sees it, del_own hides it with a tombstone
        let base = temp_dir("author");
        let foreign = seal(&secret(SEED_B), &domains_id(), b"list/x", b"v", 1_000);
        std::fs::create_dir_all(base.join("replica")).unwrap();
        file::save(
            &domains_path(&base),
            &file::encode(&domains_id(), None, &[foreign]),
            Bak::Mirror,
        )
        .unwrap();
        let d = open(&start(&base).await, "domains").await;
        let seen = must_finish(d.latest_per_key(b"list/")).await.unwrap();
        assert_eq!((seen.len(), seen[0].value.as_ref()), (1, &b"v"[..]));
        must_finish(d.del_own(b"list/x")).await.unwrap();
        let hidden = must_finish(d.latest_per_key(b"list/")).await.unwrap();
        assert!(hidden[0].is_tombstone() && hidden[0].author == d.me());
    }
}

mod corrupt {
    use super::*;

    #[tokio::test]
    async fn domains_broken_starts_empty() {
        // given: the main file and .bak of "domains" are damaged → when: start → then: Ok, the list is empty, the files are in quarantine, the second start is also Ok
        let base = temp_dir("broken");
        std::fs::create_dir_all(base.join("replica")).unwrap();
        std::fs::write(domains_path(&base), b"garbage").unwrap();
        std::fs::write(bak_of(&domains_path(&base)), b"garbage").unwrap();
        let sync = start(&base).await;
        let d = open(&sync, "domains").await;
        assert!(must_finish(d.scan(b"")).await.unwrap().is_empty());
        let broken = std::fs::read_dir(base.join("replica"))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".broken-")
            })
            .count();
        assert!(broken >= 1, "no *.broken-* file");
        drop(d);
        drop(sync);
        start(&base).await;
    }
}
