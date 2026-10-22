//! The flood test scaffold on `mem`: a node with a stack and a sync, address exchange, fast timeouts,
//! waiting for topic events. Shared by the `flood` and `node::start` tests; independent of `net-hyper`.

use super::topic::{Probed, TopicCmd};
use super::{FloodSync, Tuning};
use crate::transport::net::contract::{must_finish, secret, SyncPair, SEED_A, SEED_B, STEP};
use crate::transport::net::id::PeerId;
use crate::transport::net::mem::{MemNet, MemSyncNet};
use crate::transport::net::sync_backend::{SyncBackend, SyncCtx};
use crate::transport::net::{NetStack, NetStackBuilder, SwarmEvent, SYNC};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot};

/// The harness's shared network seed.
pub(crate) const NETWORK: [u8; 32] = [9; 32];

pub(crate) struct FloodNode {
    pub sync: Arc<FloodSync>,
    pub stack: NetStack,
}

/// Fast timeouts: silence and the handshake are measured in seconds, not tens of seconds.
pub(crate) fn fast() -> Tuning {
    Tuning {
        keepalive: Duration::from_millis(200),
        idle: Duration::from_secs(3),
        handshake: Duration::from_secs(1),
        ..Tuning::default()
    }
}

/// Node `seed`: a sync with a replica delegate on hub `hub`, a stack on `net` with the `SYNC` service, the handle attached.
pub(crate) fn mem_node(
    net: &MemNet,
    hub: &Arc<MemSyncNet>,
    seed: u8,
    network_seed: [u8; 32],
    t: Tuning,
) -> FloodNode {
    let ctx = SyncCtx {
        secret: secret(seed),
        network_seed,
        store_dir: None,
    };
    let delegate: Arc<dyn SyncBackend> = hub.backend(ctx.clone());
    let sync = FloodSync::new(&ctx, delegate, t);
    let (id, handler) = sync.service().expect("flooding has a service");
    let stack = NetStackBuilder::new(secret(seed))
        .transport(net.transport(&secret(seed)))
        .serve(id, handler)
        .build()
        .expect("the stack was built");
    assert_eq!(id, SYNC);
    sync.attach_stack(stack.downgrade());
    FloodNode { sync, stack }
}

/// The nodes know each other's addresses (without hints `mem` answers `NoRoute`).
pub(crate) fn introduce(a: &FloodNode, b: &FloodNode) {
    a.stack.add_hints(b.stack.invite_addrs()[0].clone());
    b.stack.add_hints(a.stack.invite_addrs()[0].clone());
}

/// Asks the actor of topic `topic`: links, started dials, live tasks.
pub(crate) async fn probe(node: &FloodNode, topic: [u8; 32]) -> Probed {
    let cmd = node.sync.shared.topic(&topic).expect("the topic is open");
    let (reply, answer) = oneshot::channel();
    cmd.send(TopicCmd::Probe(reply))
        .await
        .expect("the actor is alive");
    must_finish(answer).await.expect("the actor answered")
}

/// The membership contract harness: two nodes of one network with exchanged addresses.
pub(crate) async fn mem_sync_pair() -> SyncPair {
    let (net, hub) = (MemNet::new(), MemSyncNet::new());
    let a = mem_node(&net, &hub, SEED_A, NETWORK, fast());
    let b = mem_node(&net, &hub, SEED_B, NETWORK, fast());
    introduce(&a, &b);
    SyncPair {
        a: a.sync.clone(),
        b: b.sync.clone(),
        _keep: Box::new((net, hub, a, b)),
        raw: None,
    }
}

pub(crate) const TOPIC: [u8; 32] = [7; 32];
pub(crate) const QUIET: Duration = Duration::from_millis(300);

pub(crate) type Events = broadcast::Receiver<SwarmEvent>;

/// The network and the replica hub of one test.
pub(crate) struct Rig {
    pub(crate) net: Arc<MemNet>,
    pub(crate) hub: Arc<MemSyncNet>,
}

impl Rig {
    pub(crate) fn new() -> Rig {
        Rig {
            net: MemNet::new(),
            hub: MemSyncNet::new(),
        }
    }

    pub(crate) fn node(&self, seed: u8) -> FloodNode {
        self.node_with(seed, fast())
    }

    pub(crate) fn node_with(&self, seed: u8, t: Tuning) -> FloodNode {
        mem_node(&self.net, &self.hub, seed, NETWORK, t)
    }
}

/// Waits up to `STEP` for an event matching `want`; skips the others.
pub(crate) async fn next(events: &mut Events, want: impl Fn(&SwarmEvent) -> bool) -> SwarmEvent {
    let waiting = async {
        loop {
            match events.recv().await {
                Ok(event) if want(&event) => return event,
                Ok(_) => continue,
                Err(e) => panic!("the event channel is closed or lagging: {e}"),
            }
        }
    };
    match tokio::time::timeout(STEP, waiting).await {
        Ok(event) => event,
        Err(_) => panic!("the needed event did not arrive within {STEP:?}"),
    }
}

/// All events that arrived within `QUIET`.
pub(crate) async fn quiet(events: &mut Events) -> Vec<SwarmEvent> {
    let mut seen = Vec::new();
    let end = tokio::time::Instant::now() + QUIET;
    while let Ok(Ok(event)) = tokio::time::timeout_at(end, events.recv()).await {
        seen.push(event);
    }
    seen
}

pub(crate) fn is_up(id: PeerId) -> impl Fn(&SwarmEvent) -> bool {
    move |e| matches!(e, SwarmEvent::NeighborUp(p) if *p == id)
}

pub(crate) fn is_down(id: PeerId) -> impl Fn(&SwarmEvent) -> bool {
    move |e| matches!(e, SwarmEvent::NeighborDown(p) if *p == id)
}

pub(crate) fn is_received(e: &SwarmEvent) -> bool {
    matches!(e, SwarmEvent::Received { .. })
}

/// How many `Received` events are in the list.
pub(crate) fn received_count(seen: &[SwarmEvent]) -> usize {
    seen.iter().filter(|e| is_received(e)).count()
}

/// Waits for `NeighborUp` of each of `ids` in any order.
pub(crate) async fn wait_ups(events: &mut Events, ids: &[PeerId]) {
    let mut left: Vec<PeerId> = ids.to_vec();
    while !left.is_empty() {
        let up = next(events, |e| matches!(e, SwarmEvent::NeighborUp(_))).await;
        if let SwarmEvent::NeighborUp(p) = up {
            left.retain(|x| *x != p);
        }
    }
}
