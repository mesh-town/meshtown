//! The second start phase: sync, the network stack with service handlers, and the member/resolver contexts — everything
//! that ties the transports to the rest of the network. Discovery is built here too: the `invite`, `cache`
//! and `hyperdht` sources go into the stack, the `found` tap leads to the main loop.

use portable_atomic::AtomicU64;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::transport::net::{
    NetChoice, NetError, NetStack, PeerAddr, PeerId, Registry, SyncBackend, SyncCtx,
};
use anyhow::{Context, Result};
use tokio::sync::{mpsc, watch, Notify};

use crate::exit::{Clients, ExitTraffic};
use crate::members::access::view::AccessView;
use crate::members::membership::{MemberContext, MemberEvent, Members};
use crate::members::network_key::SubKeys;
use crate::members::peer_names::PeerNames;
use crate::node::output::say;
use crate::ports_sharing::PublishedPortsTable;
use crate::proxy::domains::DomainList;
use crate::site_checks::checks;
use crate::site_checks::reachability::CheckConfig;
use crate::transport::peers_cache::PeersCache;
use crate::transport::transfer_probe::ProbeSettings;

use crate::node::config::NodeConfig;

use super::boot::Boot;
use super::preload::{load_sources, preload_peer_names_into_members};
use super::stack::{build_stack, Built};

/// What `spawn_network` hands to the main loop: discovery reports, member events and the address cache.
pub(super) struct Inbound {
    pub(super) found: mpsc::Receiver<PeerAddr>,
    pub(super) events: mpsc::UnboundedReceiver<MemberEvent>,
    pub(super) cache: PeersCache,
}

/// Sync (swarm and replicas), the network stack and the member/resolver contexts — everything that ties
/// the transports to the rest of the network.
pub(super) struct Net {
    pub(super) sync_backend: Arc<dyn SyncBackend>,
    pub(super) members: Members,
    pub(super) ok_probe_count: Arc<AtomicUsize>,
    pub(super) probe_interval_secs: Arc<AtomicU64>,
    /// The list of selected exits: one source for both `RouteCtx` and the volume probe.
    pub(super) route_exits: Arc<watch::Sender<Vec<PeerId>>>,
    pub(super) ctx: MemberContext,
    pub(super) blocked_domains: Arc<HashSet<String>>,
    pub(super) check_cfg: CheckConfig,
    pub(super) resolver: Arc<crate::site_checks::doh::Resolver>,
    pub(super) exit_hosts: Arc<HashMap<String, SocketAddr>>,
    pub(super) exit_on: Arc<AtomicBool>,
    pub(super) exit_announce_wakeup: Arc<Notify>,
    pub(super) domain_list_cell: Arc<OnceLock<DomainList>>,
    pub(super) exit_clients: Clients,
    pub(super) exit_traffic: ExitTraffic,
    pub(super) stack: NetStack,
    /// The `found` tap: a clone sits in `RunningNode.introduce`; the receiver is in `Inbound`.
    pub(super) found_tx: mpsc::Sender<PeerAddr>,
    pub(super) ports: PublishedPortsTable,
    pub(super) peer_names: PeerNames,
}

/// Members, the volume probe and peer names: everything `wire_members` assembles before the stack.
pub(super) struct Wired {
    pub(super) members: Members,
    pub(super) ok_probe_count: Arc<AtomicUsize>,
    pub(super) probe_interval_secs: Arc<AtomicU64>,
    pub(super) ctx: MemberContext,
    pub(super) route_exits: Arc<watch::Sender<Vec<PeerId>>>,
    pub(super) peer_names: PeerNames,
}

/// Pieces for receiving traffic needed by the stack and `Net`: the block list, checks, resolver, exits.
pub(super) struct ServeParts {
    pub(super) blocked_domains: Arc<HashSet<String>>,
    pub(super) check_cfg: CheckConfig,
    pub(super) resolver: Arc<crate::site_checks::doh::Resolver>,
    pub(super) exit_hosts: Arc<HashMap<String, SocketAddr>>,
    pub(super) ports: PublishedPortsTable,
    pub(super) exit_clients: Clients,
    pub(super) exit_traffic: ExitTraffic,
}

/// Brings up sync and the network stack; assembles the member and resolver contexts. The member event
/// channel, the discovery report receiver and the address cache are returned in `Inbound` — their consumer
/// (`run_node_loop`) appears later.
pub(super) async fn spawn_network(cfg: &NodeConfig, boot: &Boot) -> Result<(Net, Inbound)> {
    // The sync services (`mesh` has two) go to `build_stack`, the weak stack handle comes after it.
    let sync_backend = open_sync_backend(cfg, boot).await?;
    let (wired, event_rx) = wire_members(cfg, boot);
    let (found_tx, found_rx) = mpsc::channel::<PeerAddr>(64);
    let (cache, sources) = load_sources(cfg, boot);
    let serve = serve_parts(cfg)?;
    let built = build_stack(
        cfg,
        boot,
        &wired,
        &serve,
        (sources, found_tx.clone()),
        sync_backend.services(),
    )?;
    sync_backend.attach_stack(built.stack.downgrade());
    let net = assemble_net(sync_backend, wired, serve, built, found_tx);
    Ok((
        net,
        Inbound {
            found: found_rx,
            events: event_rx,
            cache,
        },
    ))
}

/// Opens sync (swarm and replicas) and writes a line about a non-default network choice.
async fn open_sync_backend(cfg: &NodeConfig, boot: &Boot) -> Result<Arc<dyn SyncBackend>> {
    let ctx = SyncCtx {
        secret: boot.secret.clone(),
        network_seed: boot.subkeys.docs_namespace_seed,
        store_dir: cfg.docs_store_dir.clone(),
    };
    let sync = sync_registry()
        .open_sync(&cfg.settings.net_choice, ctx)
        .await
        .map_err(NetError::into_anyhow)?;
    say_custom_net(&cfg.settings.net_choice);
    Ok(sync)
}

/// The member context, the hyperdht binding and peer names from the file.
fn wire_members(cfg: &NodeConfig, boot: &Boot) -> (Wired, mpsc::UnboundedReceiver<MemberEvent>) {
    // The exit list is created here, before `RouteCtx`: the volume probe decides by it whom
    // to send 1 MiB to; `RouteCtx` then takes the same sender.
    let route_exits = Arc::new(watch::Sender::new(cfg.settings.route_exits.clone()));
    let (members, event_rx, ok_probe_count, probe_interval_secs, ctx) = build_member_context(
        access_view(cfg),
        boot.id,
        &boot.subkeys,
        boot.name.clone(),
        cfg.probe_first_delay,
        cfg.settings.probe_interval().as_secs(),
        (route_exits.subscribe(), cfg.settings.probe_full_cycle),
    );
    super::transports_hyper::admit_members_on_hyper(boot, &members);
    let peer_names = preload_peer_names_into_members(cfg, boot, &members);
    (
        Wired {
            members,
            ok_probe_count,
            probe_interval_secs,
            ctx,
            route_exits,
            peer_names,
        },
        event_rx,
    )
}

/// The block list, resolver, port table and exit clients.
fn serve_parts(cfg: &NodeConfig) -> Result<ServeParts> {
    let (blocked_domains, check_cfg, resolver) = build_resolver_context(cfg)?;
    let exit_hosts = Arc::new(cfg.exit_hosts.clone());
    let ports = PublishedPortsTable::new(cfg.settings.ports_published.clone());
    let exit_clients = Clients::default();
    let exit_traffic = ExitTraffic::default();
    Ok(ServeParts {
        blocked_domains,
        check_cfg,
        resolver,
        exit_hosts,
        ports,
        exit_clients,
        exit_traffic,
    })
}

/// Puts the assembled pieces into `Net`.
fn assemble_net(
    sync_backend: Arc<dyn SyncBackend>,
    wired: Wired,
    serve: ServeParts,
    built: Built,
    found_tx: mpsc::Sender<PeerAddr>,
) -> Net {
    Net {
        sync_backend,
        members: wired.members,
        ok_probe_count: wired.ok_probe_count,
        probe_interval_secs: wired.probe_interval_secs,
        route_exits: wired.route_exits,
        ctx: wired.ctx,
        blocked_domains: serve.blocked_domains,
        check_cfg: serve.check_cfg,
        resolver: serve.resolver,
        exit_hosts: serve.exit_hosts,
        exit_on: built.exit_on,
        exit_announce_wakeup: built.exit_announce_wakeup,
        domain_list_cell: built.domain_list_cell,
        exit_clients: serve.exit_clients,
        exit_traffic: serve.exit_traffic,
        stack: built.stack,
        found_tx,
        ports: serve.ports,
        peer_names: wired.peer_names,
    }
}

/// The node's sync registry: `"mem"` (in memory, its own hub per call) and `"mesh"` (its own membership
/// by flooding and its own replicas; the `docs-store/` of the old version is moved over or set aside by `sync::replica::import`).
pub(super) fn sync_registry() -> Registry {
    let mut registry = Registry::new();
    registry.add_sync(
        crate::members::sync::replica::NAME,
        crate::members::sync::replica::sync_ctor(None),
    );
    registry.add_sync(
        "mem",
        crate::transport::net::mem::sync_ctor(crate::transport::net::mem::MemSyncNet::new()),
    );
    registry
}

/// One log line about a non-default network choice; with the default — no line.
fn say_custom_net(net: &NetChoice) {
    if *net != crate::node::net_names::default_choice() {
        let (t, d) = (net.transports.join(", "), net.discovery.join(", "));
        let s = &net.sync;
        say(
            "net",
            format!("non-default network choice: transport {t}; discovery {d}; sync {s}"),
        );
    }
}

/// The test knob opens the view at once; a live node waits for the first verdicts.
fn access_view(cfg: &NodeConfig) -> AccessView {
    match cfg.open_list {
        true => AccessView::open(),
        false => AccessView::pending(),
    }
}

/// The member context and the volume-probe settings for a new node.
fn build_member_context(
    access: AccessView,
    id: PeerId,
    subkeys: &SubKeys,
    name: String,
    probe_first_delay: Duration,
    start_probe_interval_secs: u64,
    (exits, full_cycle): (watch::Receiver<Vec<PeerId>>, bool),
) -> (
    Members,
    mpsc::UnboundedReceiver<MemberEvent>,
    Arc<AtomicUsize>,
    Arc<AtomicU64>,
    MemberContext,
) {
    let members = Members::with_access(access);
    let (event_tx, event_rx) = mpsc::unbounded_channel::<MemberEvent>();
    let ok_probe_count = Arc::new(AtomicUsize::new(0));
    // The live probe interval handle: the same `Arc` that goes into `ProbeSettings` and into
    // `RunningNode.probe_interval_secs` at the end of `start`.
    let probe_interval_secs = Arc::new(AtomicU64::new(start_probe_interval_secs.max(1)));
    let ctx = MemberContext {
        own_id: id,
        handshake_key: subkeys.handshake_key,
        members: members.clone(),
        events: event_tx,
        probe: ProbeSettings {
            first_delay: probe_first_delay,
            interval_secs: probe_interval_secs.clone(),
            ok_probe_count: ok_probe_count.clone(),
            exits,
            full_cycle,
        },
        own_name_tx: tokio::sync::watch::channel(name).0,
    };
    (members, event_rx, ok_probe_count, probe_interval_secs, ctx)
}

/// Blocked domains, check settings and the node's shared resolver (one
/// resolver for checks, for the exit and for the direct proxy path).
fn build_resolver_context(
    cfg: &NodeConfig,
) -> Result<(
    Arc<HashSet<String>>,
    CheckConfig,
    Arc<crate::site_checks::doh::Resolver>,
)> {
    let blocked_domains: Arc<HashSet<String>> =
        Arc::new(cfg.blocked_domains.iter().cloned().collect());
    let check_cfg = CheckConfig {
        user_agent: checks::DEFAULT_USER_AGENT.to_string(),
        provider_dns: cfg.settings.provider_dns,
        geo_markers: cfg.settings.geo_markers.iter().cloned().collect(),
        blocked_domains: blocked_domains.clone(),
        ..CheckConfig::default()
    };
    let resolver = Arc::new(
        crate::site_checks::doh::Resolver::new(
            cfg.settings.doh_server,
            check_cfg.limits,
            check_cfg.user_agent.clone(),
        )
        .context("node resolver")?,
    );
    Ok((blocked_domains, check_cfg, resolver))
}

#[cfg(test)]
#[path = "network_sync_tests.rs"]
mod sync_tests;

#[cfg(test)]
#[path = "network_mesh_tests.rs"]
mod mesh_tests;
