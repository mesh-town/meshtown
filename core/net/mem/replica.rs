//! The `mem` replica: in-memory entries of a node, merging by time, live broadcast to peers and a one-off reconciliation.
//! Peers are weak references: replicas do not keep each other alive.

use super::sync_hub::MemSyncNet;
use crate::net::error::{NetError, NetResult};
use crate::net::id::PeerId;
use crate::net::replica::{ReplicaEntry, Replica, ReplicaEvent, ReplicaId, SyncCount, MAX_VALUE_BYTES};
use crate::net::stream::BoxFuture;
use crate::net::sync_backend::REPLICA_DOMAINS;
use bytes::Bytes;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const EVENT_QUEUE: usize = 256;

type Key = (PeerId, Vec<u8>);

#[derive(Default)]
struct ReplicaState {
    entries: BTreeMap<Key, ReplicaEntry>,
    syncing: bool,
    neighbors: Vec<(PeerId, Weak<MemReplica>)>,
}

impl ReplicaState {
    /// Merge: the entry with the larger time stays, on a tie the previous one.
    fn merge(&mut self, e: ReplicaEntry) -> bool {
        let key = (e.author, e.key.clone());
        match self.entries.get(&key) {
            Some(old) if old.ts_micros >= e.ts_micros => false,
            _ => {
                self.entries.insert(key, e);
                true
            }
        }
    }

    /// Time of a new own entry: the clock, but strictly later than the previous entry of the key.
    fn next_ts(&self, me: PeerId, key: &[u8]) -> u64 {
        let prev = self.entries.get(&(me, key.to_vec()));
        now_micros().max(prev.map_or(0, |p| p.ts_micros + 1))
    }

    fn live_neighbors(&self) -> Vec<Arc<MemReplica>> {
        self.neighbors
            .iter()
            .filter_map(|n| n.1.upgrade())
            .collect()
    }
}

fn now_micros() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |d| d.as_micros() as u64)
}

pub struct MemReplica {
    name: &'static str,
    seed: [u8; 32],
    me: PeerId,
    hub: Weak<MemSyncNet>,
    this: Weak<MemReplica>,
    state: Mutex<ReplicaState>,
    tx: broadcast::Sender<ReplicaEvent>,
}

impl MemReplica {
    pub(super) fn new(
        name: &'static str,
        seed: [u8; 32],
        me: PeerId,
        hub: Weak<MemSyncNet>,
    ) -> Arc<MemReplica> {
        Arc::new_cyclic(|this| MemReplica {
            name,
            seed,
            me,
            hub,
            this: this.clone(),
            state: Mutex::default(),
            tx: broadcast::channel(EVENT_QUEUE).0,
        })
    }

    fn lock(&self) -> MutexGuard<'_, ReplicaState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn emit(&self, event: ReplicaEvent) {
        let _ = self.tx.send(event);
    }

    /// An entry "arrived via reconciliation": merge; if it changed — `RemoteChange`.
    pub fn put_raw(&self, e: ReplicaEntry) {
        let changed = self.lock().merge(e);
        if changed {
            self.emit(ReplicaEvent::RemoteChange);
        }
    }

    fn merge_all(&self, entries: Vec<ReplicaEntry>) -> u64 {
        let mut state = self.lock();
        let changed = entries
            .into_iter()
            .filter(|e| state.merge(e.clone()))
            .count();
        drop(state);
        if changed > 0 {
            self.emit(ReplicaEvent::RemoteChange);
        }
        changed as u64
    }

    fn snapshot(&self) -> Vec<ReplicaEntry> {
        self.lock().entries.values().cloned().collect()
    }

    fn write_own(&self, key: &[u8], value: Bytes) {
        let mut state = self.lock();
        let ts_micros = state.next_ts(self.me, key);
        let entry = ReplicaEntry {
            author: self.me,
            key: key.to_vec(),
            value,
            ts_micros,
        };
        state.entries.insert((self.me, key.to_vec()), entry.clone());
        let neighbors = state.live_neighbors();
        drop(state);
        self.emit(ReplicaEvent::LocalChange);
        neighbors.iter().for_each(|n| n.put_raw(entry.clone()));
    }

    fn set_checked(&self, key: &[u8], value: Bytes) -> NetResult<()> {
        let bad = value.is_empty() || value.len() > MAX_VALUE_BYTES;
        if bad {
            return Err(NetError::Config(format!(
                "entry value: {} bytes, must be from 1 to {MAX_VALUE_BYTES}",
                value.len()
            )));
        }
        self.write_own(key, value);
        Ok(())
    }

    fn entries_with(&self, prefix: &[u8]) -> Vec<ReplicaEntry> {
        let state = self.lock();
        let all = state.entries.values();
        all.filter(|e| e.key.starts_with(prefix)).cloned().collect()
    }

    fn latest_with(&self, prefix: &[u8]) -> Vec<ReplicaEntry> {
        let mut best: BTreeMap<Vec<u8>, ReplicaEntry> = BTreeMap::new();
        for e in self.entries_with(prefix) {
            let rank = |x: &ReplicaEntry| (x.ts_micros, *x.author.as_bytes());
            let beats = best.get(&e.key).is_none_or(|old| rank(&e) > rank(old));
            if beats {
                best.insert(e.key.clone(), e);
            }
        }
        best.into_values().collect()
    }

    fn enable_sync(&self) -> NetResult<()> {
        if self.name == REPLICA_DOMAINS {
            return Err(NetError::Refused);
        }
        self.lock().syncing = true;
        Ok(())
    }

    fn sync_with_peer(&self, peer: PeerId) -> NetResult<()> {
        if self.name == REPLICA_DOMAINS {
            return Err(NetError::Refused);
        }
        match self.find_peer(peer) {
            Ok(other) => self.exchange(&other),
            Err(why) => self.emit(ReplicaEvent::Synced {
                peer,
                outcome: Err(why),
            }),
        }
        Ok(())
    }

    fn find_peer(&self, peer: PeerId) -> Result<Arc<MemReplica>, String> {
        if peer == self.me {
            return Err("reconciling with itself is not needed".to_string());
        }
        if !self.lock().syncing {
            return Err("reconciliation is not enabled on itself".to_string());
        }
        let hub = self.hub.upgrade();
        let other = hub.and_then(|h| h.replica(self.name, self.seed, peer));
        let other = other.ok_or_else(|| "the node has no such replica".to_string())?;
        let on = other.lock().syncing;
        match on {
            true => Ok(other),
            false => Err("reconciliation is not enabled on the node".to_string()),
        }
    }

    /// Exchange of all entries in both directions, getting acquainted and the final events.
    fn exchange(&self, other: &Arc<MemReplica>) {
        let (mine, theirs) = (self.snapshot(), other.snapshot());
        let received = self.merge_all(theirs);
        let sent = other.merge_all(mine);
        self.add_neighbor(other);
        if let Some(me) = self.this.upgrade() {
            other.add_neighbor(&me);
        }
        self.emit(synced(other.me, received, sent));
        other.emit(synced(self.me, sent, received));
    }

    fn add_neighbor(&self, other: &Arc<MemReplica>) {
        let mut state = self.lock();
        let known = state.neighbors.iter().any(|n| n.0 == other.me);
        if !known {
            state.neighbors.push((other.me, Arc::downgrade(other)));
        }
        drop(state);
        if !known {
            self.emit(ReplicaEvent::Neighbor {
                peer: other.me,
                up: true,
            });
        }
    }
}

fn synced(peer: PeerId, received: u64, sent: u64) -> ReplicaEvent {
    ReplicaEvent::Synced {
        peer,
        outcome: Ok(SyncCount { received, sent }),
    }
}

impl Replica for MemReplica {
    fn id(&self) -> ReplicaId {
        match self.name == REPLICA_DOMAINS {
            true => ReplicaId(*self.me.as_bytes()),
            false => ReplicaId(self.seed),
        }
    }

    fn me(&self) -> PeerId {
        self.me
    }

    fn set_own(&self, key: &[u8], value: Bytes) -> BoxFuture<'_, NetResult<()>> {
        let key = key.to_vec();
        Box::pin(async move { self.set_checked(&key, value) })
    }

    fn del_own(&self, key: &[u8]) -> BoxFuture<'_, NetResult<()>> {
        let key = key.to_vec();
        Box::pin(async move {
            self.write_own(&key, Bytes::new());
            Ok(())
        })
    }

    fn get(&self, author: PeerId, key: &[u8]) -> BoxFuture<'_, NetResult<Option<ReplicaEntry>>> {
        let key = key.to_vec();
        Box::pin(async move {
            let found = self.lock().entries.get(&(author, key)).cloned();
            Ok(found.filter(|e| !e.is_tombstone()))
        })
    }

    fn scan(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        let prefix = prefix.to_vec();
        Box::pin(async move { Ok(self.entries_with(&prefix)) })
    }

    fn latest_per_key(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        let prefix = prefix.to_vec();
        Box::pin(async move { Ok(self.latest_with(&prefix)) })
    }

    fn subscribe(&self) -> broadcast::Receiver<ReplicaEvent> {
        self.tx.subscribe()
    }

    fn start_sync(&self) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async move { self.enable_sync() })
    }

    fn sync_with(&self, peer: PeerId) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async move { self.sync_with_peer(peer) })
    }
}
