//! Tests of the order of the boot writes (`boot.rs`): the marker goes last, a failed write leaves no marker.
//! Child of `boot_tests.rs`: `Rig`, `setup`, `NOW` and the other helpers are there.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::net::{BoxFuture, NetError, ReplicaEvent, ReplicaId};

#[tokio::test]
async fn marker_is_written_after_the_admits() {
    // given: a long-time node with a cached neighbour and a recent author, over a replica that logs the writes
    let rig = setup();
    let (author, cached) = (pid(2), pid(3));
    rig.given_prior_life().await;
    rig.given_record_of(author, b"node/exit/a", ago(DAY));
    let spy = Spy::new(&rig);
    // when: the first boot
    boot_over(&rig, &spy, &[cached]).await.unwrap();
    // then: the witness, the migration and the promotion come first, the marker is the last write
    let keys = spy.written.lock().unwrap().clone();
    assert_eq!(keys.len(), 4, "{keys:?}");
    assert!(keys[..3].iter().all(|k| k.starts_with(b"acl/admit/")), "{keys:?}");
    assert_eq!(keys[3], BOOT_KEY);
}

#[tokio::test]
async fn failed_admit_leaves_no_marker_and_next_boot_retries() {
    // given: a long-time node with a cached neighbour and a recent author; writing an admission fails
    let rig = setup();
    let (author, cached) = (pid(2), pid(3));
    rig.given_prior_life().await;
    rig.given_record_of(author, b"node/exit/a", ago(DAY));
    let spy = Spy::new(&rig);
    spy.fail_admits.store(true, Ordering::SeqCst);
    // when: the first boot breaks off
    assert!(boot_over(&rig, &spy, &[cached]).await.is_err());
    // then: there is no marker
    assert!(rig.marker().await.is_none(), "an interrupted boot must leave no marker");
    // and when: the next start runs over a healthy replica
    let written = rig.when_boot(&[], &[cached], NOW + 60).await;
    // then: the same admissions and the marker are written, the migration is whole, the witness is promoted
    let want = admits_of(&[(cached, Via::None), (author, Via::Migration)]);
    assert_eq!(rig.admits().await, want);
    assert_eq!(written, 2);
    assert_eq!(rig.marker().await.map(|m| m.via), Some(Via::Migration));
}

#[tokio::test]
async fn failed_promotion_leaves_the_marker_without_the_flag() {
    // given: a node of the first stage with a witness for x; writing an admission fails
    let rig = setup();
    let x = pid(2);
    rig.given_marker(Via::Migration, NOW - DAY).await;
    rig.given_own_admit(x, Via::Witness).await;
    let spy = Spy::new(&rig);
    spy.fail_admits.store(true, Ordering::SeqCst);
    // when: the boot breaks off at the promotion
    assert!(boot_over(&rig, &spy, &[]).await.is_err());
    // then: the marker has no flag and the witness is as it was
    assert_eq!(rig.marker().await.map(|m| m.note), Some(String::new()));
    assert_eq!(rig.admits().await, admits_of(&[(x, Via::Witness)]));
    // and when: the next start runs over a healthy replica
    rig.when_boot(&[], &[], NOW + 60).await;
    // then: the witness is a vote and the flag is set
    assert_eq!(rig.admits().await, admits_of(&[(x, Via::None)]));
    assert_eq!(rig.marker().await.map(|m| m.note), Some(crate::access::record::BOOT_NOTE_ADMISSION.to_string()));
}

#[tokio::test]
async fn newcomer_marker_is_written_before_the_admits() {
    // given: a node with no earlier life and a cached neighbour, over a replica that logs the writes
    let rig = setup();
    let spy = Spy::new(&rig);
    // when: the first boot
    boot_over(&rig, &spy, &[pid(3)]).await.unwrap();
    // then: the marker is the first write, the admission follows
    let keys = spy.written.lock().unwrap().clone();
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert_eq!(keys[0], BOOT_KEY);
    assert!(keys[1].starts_with(b"acl/admit/"), "{keys:?}");
}

#[tokio::test]
async fn failed_boot_of_a_newcomer_leaves_it_a_newcomer() {
    // given: a node with no earlier life; writing an admission fails
    let rig = setup();
    let (author, cached) = (pid(2), pid(3));
    let spy = Spy::new(&rig);
    spy.fail_admits.store(true, Ordering::SeqCst);
    // when: the first boot breaks off
    assert!(boot_over(&rig, &spy, &[cached]).await.is_err());
    // then: the newcomer marker is already there
    assert_eq!(rig.marker().await.map(|m| m.via), Some(Via::None));
    // and when: the node writes its own data and a fresh author shows up in the shared replica
    rig.given_prior_life().await;
    rig.given_record_of(author, b"node/exit/a", ago(DAY));
    let written = rig.when_boot(&[], &[cached], NOW + 60).await;
    // then: the node is not taken for a long-time one: no migration, the marker is as it was
    assert_eq!(rig.admits().await, admits_of(&[(cached, Via::Witness)]));
    assert_eq!(written, 1);
    let marker = rig.marker().await.expect("the marker stays");
    assert_eq!((marker.via, marker.made_unix), (Via::None, NOW));
}

#[tokio::test]
async fn marker_synced_in_after_the_read_makes_the_run_start_over() {
    // given: a marker without the flag and an own witness, but the first read of the marker finds nothing
    let rig = setup();
    let x = pid(2);
    rig.given_marker(Via::Migration, NOW - DAY).await;
    rig.given_own_admit(x, Via::Witness).await;
    let late = LateMarker::new(&rig);
    let boot = AccessBoot { me: rig.me, initial: vec![], cached: vec![], now_unix: NOW };
    // when: one boot
    run_boot(&late, &*rig.own, &boot).await.unwrap();
    // then: within this run the witness became a vote and the marker carries the flag
    assert_eq!(rig.admits().await, admits_of(&[(x, Via::None)]));
    assert_eq!(rig.marker().await.map(|m| m.note), Some(crate::access::record::BOOT_NOTE_ADMISSION.to_string()));
}

#[tokio::test]
async fn real_newcomer_writes_its_marker_once() {
    // given: a node with no earlier life
    let rig = setup();
    let spy = Spy::new(&rig);
    // when: the first boot
    boot_over(&rig, &spy, &[pid(3)]).await.unwrap();
    // then: one marker write, no second pass
    let keys = spy.written.lock().unwrap().clone();
    assert_eq!(keys.iter().filter(|k| k.as_slice() == BOOT_KEY).count(), 1, "{keys:?}");
}

async fn boot_over(rig: &Rig, spy: &Spy, cached: &[PeerId]) -> NetResult<usize> {
    let boot = AccessBoot { me: rig.me, initial: vec![], cached: cached.to_vec(), now_unix: NOW };
    run_boot(spy, &*rig.own, &boot).await
}

/// A replica that delegates to the in-memory one, logs the keys written and can refuse to write admissions.
struct Spy {
    inner: Arc<MemReplica>,
    written: Mutex<Vec<Vec<u8>>>,
    fail_admits: AtomicBool,
}

impl Spy {
    fn new(rig: &Rig) -> Spy {
        Spy { inner: rig.shared.clone(), written: Mutex::new(Vec::new()), fail_admits: AtomicBool::new(false) }
    }
}

impl Replica for Spy {
    fn id(&self) -> ReplicaId {
        self.inner.id()
    }

    fn me(&self) -> PeerId {
        self.inner.me()
    }

    fn set_own(&self, key: &[u8], value: Bytes) -> BoxFuture<'_, NetResult<()>> {
        if self.fail_admits.load(Ordering::SeqCst) && key.starts_with(b"acl/admit/") {
            return Box::pin(async { Err(NetError::Config("disk is full".into())) });
        }
        self.written.lock().unwrap().push(key.to_vec());
        self.inner.set_own(key, value)
    }

    fn del_own(&self, key: &[u8]) -> BoxFuture<'_, NetResult<()>> {
        self.inner.del_own(key)
    }

    fn get(&self, author: PeerId, key: &[u8]) -> BoxFuture<'_, NetResult<Option<ReplicaEntry>>> {
        self.inner.get(author, key)
    }

    fn scan(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.inner.scan(prefix)
    }

    fn latest_per_key(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.inner.latest_per_key(prefix)
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ReplicaEvent> {
        self.inner.subscribe()
    }

    fn start_sync(&self) -> BoxFuture<'_, NetResult<()>> {
        self.inner.start_sync()
    }

    fn sync_with(&self, peer: PeerId) -> BoxFuture<'_, NetResult<()>> {
        self.inner.sync_with(peer)
    }
}

/// A replica whose first read of the boot marker finds nothing, as if the marker arrived by sync right after.
struct LateMarker {
    inner: Arc<MemReplica>,
    missed: AtomicBool,
}

impl LateMarker {
    fn new(rig: &Rig) -> LateMarker {
        LateMarker { inner: rig.shared.clone(), missed: AtomicBool::new(false) }
    }
}

impl Replica for LateMarker {
    fn id(&self) -> ReplicaId {
        self.inner.id()
    }

    fn me(&self) -> PeerId {
        self.inner.me()
    }

    fn set_own(&self, key: &[u8], value: Bytes) -> BoxFuture<'_, NetResult<()>> {
        self.inner.set_own(key, value)
    }

    fn del_own(&self, key: &[u8]) -> BoxFuture<'_, NetResult<()>> {
        self.inner.del_own(key)
    }

    fn get(&self, author: PeerId, key: &[u8]) -> BoxFuture<'_, NetResult<Option<ReplicaEntry>>> {
        if key == BOOT_KEY && !self.missed.swap(true, Ordering::SeqCst) {
            return Box::pin(async { Ok(None) });
        }
        self.inner.get(author, key)
    }

    fn scan(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.inner.scan(prefix)
    }

    fn latest_per_key(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.inner.latest_per_key(prefix)
    }

    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ReplicaEvent> {
        self.inner.subscribe()
    }

    fn start_sync(&self) -> BoxFuture<'_, NetResult<()>> {
        self.inner.start_sync()
    }

    fn sync_with(&self, peer: PeerId) -> BoxFuture<'_, NetResult<()>> {
        self.inner.sync_with(peer)
    }
}
