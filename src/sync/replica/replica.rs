//! The node's replica: in-memory state, signing its own entries, saving to disk, events.
//! Reconciliation and broadcast are in `replica_sync.rs`.

use super::entry::{seal, MAX_KEY_BYTES};
use super::host::Wiring;
use super::replica_sync::NetState;
use super::saver::Saver;
use super::state::State;
use crate::net::error::{NetError, NetResult};
use crate::net::id::{NodeSecret, PeerId};
use crate::net::replica::{ReplicaEntry, Replica, ReplicaEvent, ReplicaId, MAX_VALUE_BYTES};
use crate::net::stream::BoxFuture;
use crate::net::sync_backend::REPLICA_DOMAINS;
use bytes::Bytes;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use tokio::sync::broadcast;

/// The replica's event queue, like the iroh adapter's.
const EVENT_QUEUE: usize = 256;

pub struct MeshReplica {
    pub(super) name: &'static str,
    pub(super) id: ReplicaId,
    secret: NodeSecret,
    pub(super) state: Arc<Mutex<State>>,
    pub(super) saver: Saver,
    pub(super) events: broadcast::Sender<ReplicaEvent>,
    pub(super) wiring: Arc<Wiring>,
    pub(super) net: NetState,
    /// Itself as an `Arc`: reconciliation tasks need an owner, while the trait gives only `&self`.
    pub(super) this: Weak<MeshReplica>,
}

pub(crate) fn now_micros() -> u64 {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    since.map_or(0, |d| d.as_micros() as u64)
}

impl MeshReplica {
    pub(super) fn new(
        name: &'static str,
        secret: NodeSecret,
        state: State,
        saver: Saver,
        wiring: Arc<Wiring>,
    ) -> Arc<MeshReplica> {
        Arc::new_cyclic(|this| MeshReplica {
            name,
            id: state.id(),
            secret,
            state: Arc::new(Mutex::new(state)),
            saver,
            events: broadcast::channel(EVENT_QUEUE).0,
            wiring,
            net: NetState::default(),
            this: this.clone(),
        })
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn emit(&self, event: ReplicaEvent) {
        let _ = self.events.send(event);
    }

    /// `"domains"` is written to disk before returning, the rest is deferred.
    async fn persist(&self) -> NetResult<()> {
        if self.name == REPLICA_DOMAINS {
            return self.saver.save_now(&self.state).await;
        }
        self.saver.save_debounced(self.state.clone());
        Ok(())
    }

    /// Signs and merges its own entry; an empty value is a tombstone.
    async fn write_own(&self, key: &[u8], value: Bytes) -> NetResult<()> {
        let w = {
            let mut state = self.lock();
            let w = seal(&self.secret, &self.id, key, &value, state.next_ts_micros(now_micros()));
            state.merge(w.clone());
            w
        };
        self.persist().await?;
        self.emit(ReplicaEvent::LocalChange);
        self.push_entry(w);
        Ok(())
    }

    /// An insert without a signature "as if it arrived by reconciliation" (for the contract's `RawPut`).
    #[cfg(test)]
    pub(crate) fn put_raw(&self, e: ReplicaEntry) {
        use super::entry::WireEntry;
        let w = WireEntry {
            author: *e.author.as_bytes(),
            key: e.key,
            value: e.value.to_vec(),
            ts_micros: e.ts_micros,
            sig: [[0; 32]; 2],
        };
        if self.lock().merge(w) {
            self.emit(ReplicaEvent::RemoteChange);
        }
    }
}

fn bad_input(what: &str) -> NetError {
    NetError::Config(what.to_string())
}

fn check_limits(key: &[u8], value: &[u8]) -> NetResult<()> {
    if value.is_empty() || value.len() > MAX_VALUE_BYTES {
        return Err(bad_input(&format!("entry value: {} bytes, must be from 1 to {MAX_VALUE_BYTES}", value.len())));
    }
    check_key(key)
}

fn check_key(key: &[u8]) -> NetResult<()> {
    match key.len() > MAX_KEY_BYTES {
        true => Err(bad_input(&format!("entry key: {} bytes, at most {MAX_KEY_BYTES}", key.len()))),
        false => Ok(()),
    }
}

impl Replica for MeshReplica {
    fn id(&self) -> ReplicaId {
        self.id
    }

    fn me(&self) -> PeerId {
        self.secret.peer_id()
    }

    fn set_own(&self, key: &[u8], value: Bytes) -> BoxFuture<'_, NetResult<()>> {
        let key = key.to_vec();
        Box::pin(async move {
            check_limits(&key, &value)?;
            self.write_own(&key, value).await
        })
    }

    fn del_own(&self, key: &[u8]) -> BoxFuture<'_, NetResult<()>> {
        let key = key.to_vec();
        Box::pin(async move {
            check_key(&key)?;
            self.write_own(&key, Bytes::new()).await
        })
    }

    fn get(&self, author: PeerId, key: &[u8]) -> BoxFuture<'_, NetResult<Option<ReplicaEntry>>> {
        let found = self.lock().get(author, key);
        Box::pin(async move { Ok(found) })
    }

    fn scan(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        let found = self.lock().scan(prefix);
        Box::pin(async move { Ok(found) })
    }

    fn latest_per_key(&self, prefix: &[u8]) -> BoxFuture<'_, NetResult<Vec<ReplicaEntry>>> {
        let found = self.lock().latest_per_key(prefix);
        Box::pin(async move { Ok(found) })
    }

    fn subscribe(&self) -> broadcast::Receiver<ReplicaEvent> {
        self.events.subscribe()
    }

    fn start_sync(&self) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(self.start_sync_impl())
    }

    fn sync_with(&self, peer: PeerId) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(self.sync_with_impl(peer))
    }

    fn flush(&self) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(self.saver.save_now(&self.state))
    }
}
