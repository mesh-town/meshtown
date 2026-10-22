//! The fourth start phase: subscribing to the swarm topic, tracking peers and starting the main
//! loop of the node. Discovery (hyperdht, invite, cache) lives in the stack, the `network` phase.

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use crate::members::network_key::SubKeys;
use crate::node::config::NodeConfig;
use crate::node::event_loop::{run_node_loop, LoopCtx, LoopInputs};
use crate::transport::net::{Membership, NetError, PeerId};
use anyhow::Result;

use crate::node::guard::GuardHandle;

use super::boot::Boot;
use super::network::{Inbound, Net};
use super::serving::Serving;

/// The result of `wire_entry_points`, needed by `start_node()` to assemble `RunningNode`.
pub(super) struct EntryPoints {
    pub gossip_neighbor_count: Arc<AtomicUsize>,
    /// A snapshot of the address cache at startup (id → `seen_unix`); empty if the cache is off or empty.
    pub cached_peers: Arc<HashMap<PeerId, u64>>,
    /// The handle of the memory-guard thread (`None` — the guard is off); stopped by `RunningNode::shutdown`.
    pub memory_guard: Option<GuardHandle>,
}

/// Subscribes to the swarm topic, tracks peers and starts the node's main loop.
pub(super) async fn wire_entry_points(
    cfg: &NodeConfig,
    boot: &mut Boot,
    net: &Net,
    serving: &Serving,
    inbound: Inbound,
) -> Result<EntryPoints> {
    let gossip_neighbor_count = Arc::new(AtomicUsize::new(0));
    let swarm = subscribe_gossip_topic(
        net,
        &boot.subkeys,
        &gossip_neighbor_count,
        &mut boot.background_abort_handles,
    )
    .await?;

    let cached_peers = Arc::new(inbound.cache.seen());

    let memory_guard = spawn_memory_guard(cfg, net, serving);
    let run = tokio::spawn(run_node_loop(
        loop_ctx(boot, net, serving, swarm, gossip_neighbor_count.clone()),
        loop_inputs(net, inbound),
    ));
    boot.background_abort_handles.push(run.abort_handle());

    Ok(EntryPoints {
        gossip_neighbor_count,
        cached_peers,
        memory_guard,
    })
}

/// The node's memory guard (`RunningNode` keeps the handle, `shutdown` stops the thread).
fn spawn_memory_guard(cfg: &NodeConfig, net: &Net, serving: &Serving) -> Option<GuardHandle> {
    crate::node::guard::start_memory_guard(
        crate::node::guard::MemoryLimitsKib::from_mib(cfg.memory.warn_mib, cfg.memory.exit_mib),
        net.members.clone(),
        serving.checks.clone(),
        tokio::runtime::Handle::current(),
        heap_dump_dir(cfg),
    )
}

/// Shared handles of the node's main loop.
fn loop_ctx(
    boot: &Boot,
    net: &Net,
    serving: &Serving,
    swarm: Arc<dyn Membership>,
    gossip_neighbor_count: Arc<AtomicUsize>,
) -> LoopCtx {
    LoopCtx {
        id: boot.id,
        stack: net.stack.clone(),
        members: net.members.clone(),
        swarm,
        gossip_neighbor_count,
        domains: serving.domain_list.clone(),
        checks: serving.checks.clone(),
        telemetry: boot.telemetry.clone(),
        route: serving.route.clone(),
    }
}

/// What the main loop takes from the `network` phase.
fn loop_inputs(net: &Net, inbound: Inbound) -> LoopInputs {
    LoopInputs {
        ctx: net.ctx.clone(),
        found: inbound.found,
        events: inbound.events,
        cache: inbound.cache,
        names: net.peer_names.clone(),
    }
}

/// Subscribes to the network's swarm topic and starts peer tracking.
async fn subscribe_gossip_topic(
    net: &Net,
    subkeys: &SubKeys,
    gossip_neighbor_count: &Arc<AtomicUsize>,
    background_abort_handles: &mut Vec<tokio::task::AbortHandle>,
) -> Result<Arc<dyn Membership>> {
    let swarm = net
        .sync_backend
        .join_swarm(subkeys.gossip_topic, &[])
        .await
        .map_err(NetError::into_anyhow)?;
    background_abort_handles.extend(swarm.pump);
    background_abort_handles.push(
        tokio::spawn(crate::members::membership::run_swarm_events(
            swarm.events,
            gossip_neighbor_count.clone(),
        ))
        .abort_handle(),
    );
    Ok(swarm.membership)
}

/// The node data directory for heap-profile dumps: next to the address cache or the peer names,
/// otherwise the current directory.
fn heap_dump_dir(cfg: &NodeConfig) -> std::path::PathBuf {
    let beside = |p: &Option<std::path::PathBuf>| {
        p.as_deref()
            .and_then(|f| f.parent())
            .map(|d| d.to_path_buf())
    };
    beside(&cfg.peers_cache_path)
        .or_else(|| beside(&cfg.peer_names_path))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(test)]
#[path = "entry_tests.rs"]
mod tests;
