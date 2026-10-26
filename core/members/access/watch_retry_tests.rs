//! Tests of the retry of the recompute: a failed scan publishes nothing and the next tick tries again, and a
//! failure never erases a view that was already published.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::*;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::{NetError, NetResult, ReplicaId};

/// A replica over a memory one whose `scan` fails while the flag is on.
struct FlakyScan {
    inner: Arc<MemReplica>,
    failing: AtomicBool,
    scans: AtomicUsize,
}

impl FlakyScan {
    fn new(inner: Arc<MemReplica>, failing: bool) -> Arc<FlakyScan> {
        Arc::new(FlakyScan {
            inner,
            failing: AtomicBool::new(failing),
            scans: AtomicUsize::new(0),
        })
    }

    fn set_failing(&self, on: bool) {
        self.failing.store(on, Ordering::SeqCst);
    }
}

impl Replica for FlakyScan {
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
        self.inner.get(author, key)
    }

    fn scan(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.scans.fetch_add(1, Ordering::SeqCst);
        match self.failing.load(Ordering::SeqCst) {
            true => Box::pin(async { Err(NetError::Config("scan failed".into())) }),
            false => self.inner.scan(prefix),
        }
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

/// A rig whose shared replica can fail its scans; the list has one neighbour.
struct Flaky {
    rig: Rig,
    flaky: Arc<FlakyScan>,
    access: Arc<Access>,
}

fn flaky_setup(failing: bool) -> Flaky {
    let rig = setup();
    rig.given_admit(rig.me, pid(2));
    let flaky = FlakyScan::new(rig.shared.clone(), failing);
    let own = rig.access.own.clone();
    let access = Arc::new(Access::from_parts(
        rig.me,
        flaky.clone(),
        own,
        rig.members.clone(),
        rig.me,
    ));
    Flaky { rig, flaky, access }
}

#[tokio::test]
async fn failed_first_recompute_leaves_the_view_pending() {
    // given: a scan that fails and a view that is pending
    let f = flaky_setup(true);
    // when: the recompute runs
    let result = f.access.refresh().await;
    // then: it reports the failure and publishes nothing
    assert!(result.is_err(), "the recompute must report the failed scan");
    assert!(
        f.rig.view().verdicts().is_none(),
        "a failed recompute must not publish a view"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), f.rig.view().ready())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn next_tick_publishes_the_view_after_a_failed_first_recompute() {
    // given: a scan that fails, and a watcher with a short timer
    let f = flaky_setup(true);
    let task = crate::node::output::spawn(watch_every(f.access.clone(), Duration::from_millis(50)));
    // the first recompute has failed, and the view is still pending
    eventually("several failed scans", || {
        f.flaky.scans.load(Ordering::SeqCst) >= 3
    })
    .await;
    assert!(
        f.rig.view().verdicts().is_none(),
        "nothing may be published while the scan fails"
    );
    // when: the scan works again
    f.flaky.set_failing(false);
    // then: a later tick publishes the view and lets the handshakes continue
    eventually("a published view", || f.rig.view().verdicts().is_some()).await;
    assert_eq!(f.rig.view().check(pid(2)), Ok(()));
    within(f.rig.view().ready()).await;
    task.abort();
}

#[tokio::test]
async fn failed_recompute_keeps_the_previous_view() {
    // given: a published view
    let f = flaky_setup(false);
    f.access
        .refresh()
        .await
        .expect("the first recompute must work");
    assert_eq!(f.rig.view().check(pid(2)), Ok(()));
    // when: the scan fails and the recompute runs again
    f.flaky.set_failing(true);
    assert!(f.access.refresh().await.is_err());
    // then: the previous view stays
    assert!(
        f.rig.view().verdicts().is_some(),
        "a failed recompute must not erase the view"
    );
    assert_eq!(f.rig.view().check(pid(2)), Ok(()));
}
