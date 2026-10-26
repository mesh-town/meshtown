//! Building and starting the node: `start` ties all modules into a working process (transports, sync, the network stack with
//! discovery, background tasks) and returns `RunningNode`.
//!
//! Files: `boot` (transports, name, identity) → `network` (sync, network stack, discovery) →
//! `serving` (domain list, checks, proxy configuration) → `entry` (swarm topic, main loop). The intermediate bundles (`Boot`, `Net`, `Serving`) are visible only inside `start`.

use std::future::Future;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;

use anyhow::Result;

use crate::transport::net::NetStack;

use super::config::NodeConfig;
use super::running::RunningNode;

mod access_wiring;
mod boot;
mod entry;
mod network;
mod preload;
mod proxy_wiring;
mod serving;
mod stack;
mod transports;
mod transports_hyper;
pub(in crate::node) mod transports_mem;

use boot::boot_endpoint;
use entry::wire_entry_points;
use network::spawn_network;
use serving::setup_serving;
pub use transports::bind_ephemeral;

pub async fn start_node(cfg: NodeConfig) -> Result<RunningNode> {
    let mut boot = boot_endpoint(&cfg).await?;
    let (net, inbound) = spawn_network(&cfg, &boot).await?;
    let (serving, entry_points) =
        close_on_err(&net.stack, wire_up(&cfg, &mut boot, &net, inbound)).await?;
    let face = proxy_wiring::face_of(cfg.proxy_face.clone());
    Ok(assemble_running(boot, net, serving, entry_points, face))
}

/// Awaits a phase; a phase error closes the stack and is returned as is, success leaves the stack alone.
/// The stack lives shorter than the process: an error in later phases closes it rather than leaving it to `Drop`.
async fn close_on_err<T>(stack: &NetStack, phase: impl Future<Output = Result<T>>) -> Result<T> {
    let done = phase.await;
    if done.is_err() {
        stack.close().await;
    }
    done
}

/// Assembles `RunningNode` from the phase bundles.
fn assemble_running(
    boot: boot::Boot,
    net: network::Net,
    serving: serving::Serving,
    entry_points: entry::EntryPoints,
    proxy_face: tokio::sync::watch::Receiver<crate::proxy::ProxyFace>,
) -> RunningNode {
    RunningNode {
        id: boot.id,
        stack: net.stack,
        members: net.members,
        found_tx: net.found_tx,
        gossip_neighbor_count: entry_points.gossip_neighbor_count,
        ok_probe_count: net.ok_probe_count,
        domains: serving.domain_list,
        access: serving.access,
        checks: serving.checks,
        proxy: serving.proxy,
        proxy_face,
        exit_on: net.exit_on,
        exit_clients: net.exit_clients,
        exit_traffic: net.exit_traffic,
        route: serving.route,
        name_at_start: boot.name,
        own_name_tx: net.ctx.own_name_tx.clone(),
        identity: boot.identity,
        network_key: boot.network_key,
        net_root: boot.net_root,
        reply_secret: boot.reply_secret,
        network_label: boot.network_label,
        telemetry: boot.telemetry,
        cached_peers: entry_points.cached_peers,
        probe_interval_secs: net.probe_interval_secs,
        exit_announce_wakeup: net.exit_announce_wakeup,
        ports: net.ports,
        ports_doc: serving.ports_doc,
        doc_changes: serving.doc_changes,
        opener: serving.opener,
        background_abort_handles: Arc::new(boot.background_abort_handles),
    }
}

/// The phases after the network: services and entry points. A separate function so `start` can close the stack
/// in one place on any of their errors.
async fn wire_up(
    cfg: &NodeConfig,
    boot: &mut boot::Boot,
    net: &network::Net,
    inbound: network::Inbound,
) -> Result<(serving::Serving, entry::EntryPoints)> {
    let access = access_wiring::access_start(cfg, boot.id, &inbound.cache);
    let serving = setup_serving(cfg, boot, net, access).await?;
    let entry_points = wire_entry_points(boot, net, &serving, inbound).await?;
    Ok((serving, entry_points))
}

#[cfg(test)]
#[path = "start_tests.rs"]
mod tests;
