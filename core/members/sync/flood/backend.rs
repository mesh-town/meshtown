//! The "mesh" sync: own topics by flooding, replicas go to the inner delegate sync.

use super::admit::admission_key;
use super::handle::TopicHandle;
use super::service::FloodService;
use super::topic::{spawn_topic, TopicCmd, TopicCtx};
use super::tuning::Tuning;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::PeerId;
use crate::transport::net::replica::Replica;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::sync_backend::{Swarm, SyncCtor, SyncCtx};
use crate::transport::net::{Membership, ServiceHandler, ServiceId, SyncBackend, WeakStack, SYNC};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use tokio::sync::mpsc;

/// The sync's name in `config.json` (`net.sync`).
pub const NAME: &str = "mesh";

/// What the sync, the service and the topics share.
pub struct FloodShared {
    pub(super) me: PeerId,
    pub(super) admission_key: [u8; 32],
    pub(super) stack: Arc<OnceLock<WeakStack>>,
    pub(super) topics: Mutex<HashMap<[u8; 32], mpsc::Sender<TopicCmd>>>,
    pub(super) tuning: Tuning,
}

impl FloodShared {
    fn lock(&self) -> MutexGuard<'_, HashMap<[u8; 32], mpsc::Sender<TopicCmd>>> {
        self.topics.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The command channel of a live topic.
    pub(super) fn topic(&self, topic: &[u8; 32]) -> Option<mpsc::Sender<TopicCmd>> {
        self.lock().get(topic).filter(|tx| !tx.is_closed()).cloned()
    }
}

pub struct FloodSync {
    pub(super) shared: Arc<FloodShared>,
    replicas: Arc<dyn SyncBackend>,
    service: Arc<FloodService>,
}

impl FloodSync {
    pub fn new(ctx: &SyncCtx, replicas: Arc<dyn SyncBackend>, tuning: Tuning) -> Arc<FloodSync> {
        let shared = Arc::new(FloodShared {
            me: ctx.secret.peer_id(),
            admission_key: admission_key(&ctx.network_seed),
            stack: Arc::new(OnceLock::new()),
            topics: Mutex::default(),
            tuning,
        });
        let service = Arc::new(FloodService {
            shared: shared.clone(),
        });
        Arc::new(FloodSync {
            shared,
            replicas,
            service,
        })
    }

    /// Opens a topic and starts the actor; a busy topic is `Config`.
    fn open_topic(&self, topic: [u8; 32]) -> NetResult<Swarm> {
        // the check and the insert are under one lock; dead channels are removed along the way
        let mut topics = self.shared.lock();
        topics.retain(|_, tx| !tx.is_closed());
        if topics.contains_key(&topic) {
            return Err(NetError::Config("topic is already open".into()));
        }
        let ctx = TopicCtx {
            me: self.shared.me,
            topic,
            admission_key: self.shared.admission_key,
            stack: self.shared.stack.clone(),
            tuning: self.shared.tuning.clone(),
        };
        let (cmd, events, abort) = spawn_topic(ctx);
        let handle = TopicHandle {
            cmd: cmd.clone(),
            events,
            stack: self.shared.stack.clone(),
        };
        topics.insert(topic, cmd);
        Ok(Swarm {
            events: handle.subscribe(),
            membership: Arc::new(handle),
            pump: Some(abort),
        })
    }
}

impl FloodSync {
    /// Entry failed: stop the actor, release the topic.
    fn drop_topic(&self, topic: &[u8; 32], swarm: &Swarm) {
        swarm.pump.iter().for_each(|h| h.abort());
        self.shared.lock().remove(topic);
    }
}

impl SyncBackend for FloodSync {
    fn name(&self) -> &'static str {
        NAME
    }

    fn join_swarm(&self, topic: [u8; 32], bootstrap: &[PeerId]) -> BoxFuture<'_, NetResult<Swarm>> {
        let bootstrap = bootstrap.to_vec();
        Box::pin(async move {
            let swarm = self.open_topic(topic)?;
            let joined = swarm.membership.join(&bootstrap).await;
            joined.inspect_err(|_| self.drop_topic(&topic, &swarm))?;
            Ok(swarm)
        })
    }

    fn open_replica(&self, name: &str) -> BoxFuture<'_, NetResult<Arc<dyn Replica>>> {
        self.replicas.open_replica(name)
    }

    fn service(&self) -> Option<(ServiceId, Arc<dyn ServiceHandler>)> {
        Some((SYNC, self.service.clone()))
    }

    fn attach_stack(&self, stack: WeakStack) {
        let _ = self.shared.stack.set(stack);
    }
}

/// The constructor for `Registry::add_sync("mesh", …)`: takes replicas from `replicas`.
pub fn sync_ctor(replicas: SyncCtor) -> SyncCtor {
    let inner = Arc::new(replicas);
    Box::new(move |ctx| {
        let inner = inner.clone();
        Box::pin(async move {
            let delegate = inner(ctx.clone()).await?;
            let sync = FloodSync::new(&ctx, delegate, Tuning::default());
            Ok(sync as Arc<dyn SyncBackend>)
        })
    })
}
