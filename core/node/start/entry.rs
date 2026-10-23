//! The fourth start phase: subscribing to the swarm topic, tracking peers and starting the main
//! loop of the node. Discovery (hyperdht, invite, cache) lives in the stack, the `network` phase.

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use crate::members::network_key::SubKeys;
use crate::node::event_loop::{run_node_loop, LoopCtx, LoopInputs};
use crate::transport::net::{Membership, NetError, PeerId};
use anyhow::Result;

use super::boot::Boot;
use super::network::{Inbound, Net};
use super::serving::Serving;

/// The result of `wire_entry_points`, needed by `start_node()` to assemble `RunningNode`.
pub(super) struct EntryPoints {
    pub gossip_neighbor_count: Arc<AtomicUsize>,
    /// A snapshot of the address cache at startup (id → `seen_unix`); empty if the cache is off or empty.
    pub cached_peers: Arc<HashMap<PeerId, u64>>,
}

/// Subscribes to the swarm topic, tracks peers and starts the node's main loop.
pub(super) async fn wire_entry_points(
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

    let run = crate::node::output::spawn(run_node_loop(
        loop_ctx(boot, net, serving, swarm, gossip_neighbor_count.clone()),
        loop_inputs(net, inbound),
    ));
    boot.background_abort_handles.push(run.abort_handle());

    Ok(EntryPoints {
        gossip_neighbor_count,
        cached_peers,
    })
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
        crate::node::output::spawn(crate::members::membership::run_swarm_events(
            swarm.events,
            gossip_neighbor_count.clone(),
        ))
        .abort_handle(),
    );
    Ok(swarm.membership)
}
