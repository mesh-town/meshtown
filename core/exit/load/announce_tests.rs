//! Tests of the announcer's table on in-memory replicas; time is passed as an argument.

use super::*;
use crate::exit::load::board::read_exit_loads;
use crate::node::node_state::{read_exit_flags, EXIT_KEY};
use crate::transport::net::mem::{MemReplica, MemSyncNet};
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_NET};
use crate::transport::net::{
    NetError, NetResult, NodeSecret, PeerId, ReplicaEntry, ReplicaEvent, ReplicaId,
};
use bytes::Bytes;
use std::sync::Arc;

/// A node, its announcer and a clock that starts now and moves only when told to.
struct Rig {
    rep: Arc<MemReplica>,
    announcer: Announcer,
    start: Instant,
    wall_us: u64,
}

impl Rig {
    fn new() -> Self {
        let rep = replica(1);
        let wall_us = Tick::now().wall_us;
        Self {
            rep,
            announcer: Announcer::default(),
            start: Instant::now(),
            wall_us,
        }
    }

    /// One step at `secs` seconds after the start.
    async fn step(&mut self, secs: u64, want: Option<Reason>) -> Action {
        let at = Duration::from_secs(secs);
        let tick = Tick {
            mono: self.start + at,
            wall_us: self.wall_us + at.as_micros() as u64,
        };
        self.announcer.step(&*self.rep, want, tick).await.unwrap()
    }

    async fn record(&self) -> Option<(Bytes, u64)> {
        own_value(&*self.rep, KEY).await.unwrap()
    }
}

#[tokio::test]
async fn announce_writes_reason_and_clear_writes_tombstone() {
    // given: a fresh node
    let mut rig = Rig::new();
    // when: overloaded by the cap
    rig.step(0, Some(Reason::Limit)).await;
    // then: the record holds the word
    assert_eq!(rig.record().await.unwrap().0, Bytes::from_static(b"limit"));
    // when: the overload is gone a minute later
    rig.step(60, None).await;
    // then: a tombstone
    assert_eq!(rig.record().await, None);
    let all = rig.rep.scan(KEY).await.unwrap();
    assert!(
        all.iter().any(|e| e.is_tombstone()),
        "no tombstone: {all:?}"
    );
}

#[tokio::test]
async fn no_two_writes_within_a_minute() {
    // given: a record written at 0
    let mut rig = Rig::new();
    rig.step(0, Some(Reason::Share)).await;
    // when: the overload is gone at 20 s
    let early = rig.step(20, None).await;
    // then: nothing is written yet; at 60 s the tombstone goes
    assert_eq!(early, Action::Nothing);
    assert!(rig.record().await.is_some());
    assert_eq!(rig.step(60, None).await, Action::Clear);
}

#[tokio::test]
async fn refresh_rewrites_after_ten_minutes() {
    // given: a record written at 0
    let mut rig = Rig::new();
    rig.step(0, Some(Reason::Share)).await;
    let first = rig.record().await.unwrap().1;
    // when: the reason changes before ten minutes
    let early = rig.step(599, Some(Reason::Memory)).await;
    // then: nothing is written
    assert_eq!(early, Action::Nothing);
    assert_eq!(rig.record().await.unwrap().1, first);
    // when: ten minutes pass
    rig.step(601, Some(Reason::Memory)).await;
    // then: rewritten with the current reason and a newer time
    let (value, ts) = rig.record().await.unwrap();
    assert_eq!(value, Bytes::from_static(b"memory"));
    assert!(ts > first, "{ts} <= {first}");
}

#[tokio::test]
async fn record_left_from_last_run_is_cleared_on_first_step() {
    // given: a record from the previous run, and the state "not overloaded"
    let mut rig = Rig::new();
    rig.rep
        .set_own(KEY, Bytes::from_static(b"limit"))
        .await
        .unwrap();
    // when: the first step
    let action = rig.step(0, None).await;
    // then: a tombstone
    assert_eq!(action, Action::Clear);
    assert_eq!(rig.record().await, None);
}

#[tokio::test]
async fn record_restored_by_sync_is_cleared() {
    // given: a step with nothing to do
    let mut rig = Rig::new();
    assert_eq!(rig.step(0, None).await, Action::Nothing);
    // when: sync brings our own old record back
    let me = rig.rep.me();
    rig.rep
        .put_raw(entry(me, KEY, b"share", Tick::now().wall_us));
    // then: the next step clears it
    assert_eq!(rig.step(15, None).await, Action::Clear);
    assert_eq!(rig.record().await, None);
}

#[tokio::test]
async fn exit_off_clears_record() {
    // given: a record, written while the exit was on
    let mut rig = Rig::new();
    rig.step(0, Some(Reason::Limit)).await;
    // when: the exit is off (the runner passes `None`)
    let action = rig.step(60, None).await;
    // then: cleared
    assert_eq!(action, Action::Clear);
}

#[tokio::test]
async fn failed_write_is_retried_next_step() {
    // given: a replica whose writes fail
    let mut rig = Rig::new();
    let broken: Arc<dyn Replica> = Arc::new(FailingReplica(rig.rep.clone()));
    let tick = Tick {
        mono: rig.start,
        wall_us: rig.wall_us,
    };
    // when: the write fails
    let failed = rig
        .announcer
        .step(&*broken, Some(Reason::Limit), tick)
        .await;
    assert!(failed.is_err());
    // then: nothing is recorded, and the next step (15 s later, well inside the gap) writes
    assert_eq!(rig.record().await, None);
    assert_eq!(
        rig.step(15, Some(Reason::Limit)).await,
        Action::Write(Reason::Limit)
    );
}

#[tokio::test]
async fn same_value_gets_newer_stamp() {
    // given: a record
    let rig = Rig::new();
    rig.rep
        .set_own(KEY, Bytes::from_static(b"share"))
        .await
        .unwrap();
    let first = rig.record().await.unwrap().1;
    // when: the same value is written again
    rig.rep
        .set_own(KEY, Bytes::from_static(b"share"))
        .await
        .unwrap();
    // then: the stamp is larger
    assert!(rig.record().await.unwrap().1 > first);
}

#[tokio::test]
async fn record_reaches_third_node_through_one_that_ignores_key() {
    // given: three nodes; B reads nothing of this key and only passes records on
    let mut a = Rig::new();
    let (b, c) = (replica(2), replica(3));
    a.step(0, Some(Reason::Memory)).await;
    // when: the record travels A -> B -> C as an opaque entry
    let from_a = a.rep.get(a.rep.me(), KEY).await.unwrap().unwrap();
    b.put_raw(from_a);
    let at_b = b.get(a.rep.me(), KEY).await.unwrap().unwrap();
    c.put_raw(at_b);
    // then: C reads it
    let loads = read_exit_loads(&*c).await.unwrap();
    assert_eq!(
        loads.get(&a.rep.me()).map(|l| l.cause),
        Some(super::super::board::Cause::Known(Reason::Memory))
    );
}

#[tokio::test]
async fn exit_flag_readers_do_not_see_the_new_key() {
    // given: a node with the exit flag key, then the overload record
    let mut rig = Rig::new();
    rig.rep
        .set_own(EXIT_KEY, Bytes::from_static(b"on"))
        .await
        .unwrap();
    let before = read_exit_flags(&*rig.rep).await.unwrap();
    // when: the record is written
    rig.step(0, Some(Reason::Limit)).await;
    // then: the flags are the same
    assert_eq!(read_exit_flags(&*rig.rep).await.unwrap(), before);
}

#[test]
fn plan_follows_the_table() {
    // given / when / then: the five rows
    let s = Some(Reason::Share);
    assert_eq!(plan(s, None), Action::Write(Reason::Share));
    assert_eq!(plan(s, Some(Duration::from_secs(599))), Action::Nothing);
    assert_eq!(
        plan(s, Some(Duration::from_secs(600))),
        Action::Write(Reason::Share)
    );
    assert_eq!(plan(None, Some(Duration::ZERO)), Action::Clear);
    assert_eq!(plan(None, None), Action::Nothing);
    assert_eq!(age_of(10, 5), Duration::ZERO);
}

fn replica(seed: u8) -> Arc<MemReplica> {
    let ctx = SyncCtx {
        secret: NodeSecret::from_bytes(&[seed; 32]),
        network_seed: [9; 32],
        store_dir: None,
    };
    MemSyncNet::new()
        .backend(ctx)
        .replica_mem(REPLICA_NET)
        .unwrap()
}

fn entry(author: PeerId, key: &[u8], value: &'static [u8], ts_micros: u64) -> ReplicaEntry {
    ReplicaEntry {
        author,
        key: key.to_vec(),
        value: Bytes::from_static(value),
        ts_micros,
    }
}

/// A replica whose writes fail; reads go to the inner one.
struct FailingReplica(Arc<MemReplica>);

impl Replica for FailingReplica {
    fn id(&self) -> ReplicaId {
        self.0.id()
    }
    fn me(&self) -> PeerId {
        self.0.me()
    }
    fn set_own(&self, _: &[u8], _: Bytes) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async { Err(NetError::Closed) })
    }
    fn del_own(&self, _: &[u8]) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async { Err(NetError::Closed) })
    }
    fn get(&self, a: PeerId, k: &[u8]) -> BoxFuture<'_, NetResult<Option<ReplicaEntry>>> {
        self.0.get(a, k)
    }
    fn scan(&self, p: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.0.scan(p)
    }
    fn latest_per_key(&self, p: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        self.0.latest_per_key(p)
    }
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ReplicaEvent> {
        self.0.subscribe()
    }
    fn start_sync(&self) -> BoxFuture<'_, NetResult<()>> {
        self.0.start_sync()
    }
    fn sync_with(&self, p: PeerId) -> BoxFuture<'_, NetResult<()>> {
        self.0.sync_with(p)
    }
}
