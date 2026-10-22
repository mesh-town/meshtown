//! Sync as a swappable part: topic membership and named replicas behind one trait.

use crate::transport::net::error::NetResult;
use crate::transport::net::facade::{ServiceHandler, WeakStack};
use crate::transport::net::id::{NodeSecret, PeerId};
use crate::transport::net::replica::Replica;
use crate::transport::net::service::ServiceId;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::sync::{Membership, SwarmEvent};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::AbortHandle;

/// A replica that reconciles with network members.
pub const REPLICA_NET: &str = "net";
/// Own domain list: never reconciled.
pub const REPLICA_DOMAINS: &str = "domains";

/// What the sync needs at creation.
#[derive(Clone, Debug)]
pub struct SyncCtx {
    pub secret: NodeSecret,
    pub network_seed: [u8; 32],
    /// `None` — in memory.
    pub store_dir: Option<PathBuf>,
}

/// Broadcast topic: membership, event stream and a pump task (it can be stopped).
pub struct Swarm {
    pub membership: Arc<dyn Membership>,
    pub events: broadcast::Receiver<SwarmEvent>,
    pub pump: Option<AbortHandle>,
}

/// Lifecycle: constructor → `services` (or `service`) → stack assembly → `attach_stack`.
pub trait SyncBackend: Send + Sync {
    fn name(&self) -> &'static str;
    /// Joins the broadcast topic with the initial peers.
    fn join_swarm(&self, topic: [u8; 32], bootstrap: &[PeerId]) -> BoxFuture<'_, NetResult<Swarm>>;
    /// Opens a replica by name; any other name gives `Err(Config)`; does not turn reconciliation on.
    fn open_replica(&self, name: &str) -> BoxFuture<'_, NetResult<Arc<dyn Replica>>>;
    /// Sync service handler for the stack builder; `None` — there is no service of its own.
    fn service(&self) -> Option<(ServiceId, Arc<dyn ServiceHandler>)> {
        None
    }
    /// All sync service handlers for the stack builder; the default is the single `service()`.
    fn services(&self) -> Vec<(ServiceId, Arc<dyn ServiceHandler>)> {
        self.service().into_iter().collect()
    }
    /// Weak handle of the assembled stack (after assembly); the default does nothing.
    fn attach_stack(&self, _stack: WeakStack) {}
}

pub type SyncCtor =
    Box<dyn Fn(SyncCtx) -> BoxFuture<'static, NetResult<Arc<dyn SyncBackend>>> + Send + Sync>;
