//! Building the node's `NetStack`: transports, services, discovery sources. The replica and members are in `network`.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};

use crate::transport::net::{
    service as svc, Discovery, NetError, NetStack, NetStackBuilder, PeerAddr, ServiceHandler,
};
use anyhow::Result;
use tokio::sync::{mpsc, Notify};

use crate::exit::ExitConfig;
use crate::proxy::domains::DomainList;

use super::boot::Boot;
use super::network::{ServeParts, Wired};
use crate::node::config::NodeConfig;

/// The discovery sources and the `found` tap that feeds the node's main loop.
pub(super) type Finding = (Vec<Arc<dyn Discovery>>, mpsc::Sender<PeerAddr>);

/// The sync services for `serve` (`mesh` has two, `mem` none).
pub(super) type SyncServices = Vec<(svc::ServiceId, Arc<dyn ServiceHandler>)>;

/// The result of `build_stack`: the exit flag, its announcer's alarm, the domain-list cell for
/// the exit protocol and the assembled network stack.
pub(super) struct Built {
    pub(super) exit_on: Arc<AtomicBool>,
    pub(super) exit_announce_wakeup: Arc<Notify>,
    pub(super) domain_list_cell: Arc<OnceLock<DomainList>>,
    pub(super) stack: NetStack,
}

/// Builds the node's network stack (see `Built`).
pub(super) fn build_stack(
    cfg: &NodeConfig,
    boot: &Boot,
    wired: &Wired,
    serve: &ServeParts,
    finding: Finding,
    sync_svcs: SyncServices,
) -> Result<Built> {
    let exit_on = Arc::new(AtomicBool::new(cfg.settings.exit_enabled));
    // The alarm of the exit-flag announcer: `set_exit_enabled` calls `notify_one`
    // from sync code; with no waiter it keeps one permit until the next poll.
    let exit_announce_wakeup = Arc::new(Notify::new());
    // The domain list opens later, but the exit handler is needed by the stack right now: we give
    // it a cell that we fill right after the list opens.
    let domain_list_cell: Arc<OnceLock<DomainList>> = Arc::new(OnceLock::new());
    let exit_cfg = ExitConfig {
        enabled: exit_on.clone(),
        members: wired.members.clone(),
        list: domain_list_cell.clone(),
        resolver: serve.resolver.clone(),
        host_overrides: serve.exit_hosts.clone(),
        clients: serve.exit_clients.clone(),
        traffic: serve.exit_traffic.clone(),
    };
    let builder = served_builder(cfg, boot, wired, serve, exit_cfg, &domain_list_cell);
    let stack = with_discovery(builder, finding, sync_svcs)?;
    Ok(Built {
        exit_on,
        exit_announce_wakeup,
        domain_list_cell,
        stack,
    })
}

/// Transports and handlers of the AUTH, EXIT, PORT, INVITE services.
fn served_builder(
    cfg: &NodeConfig,
    boot: &Boot,
    wired: &Wired,
    serve: &ServeParts,
    exit_cfg: ExitConfig,
    domain_list_cell: &Arc<OnceLock<DomainList>>,
) -> NetStackBuilder {
    boot.transports
        .iter()
        .fold(NetStackBuilder::new(boot.secret.clone()), |b, t| {
            b.transport(t.clone())
        })
        .serve(
            svc::AUTH,
            crate::members::handshake::handshake_service(wired.ctx.clone()),
        )
        .serve(svc::EXIT, crate::exit::exit_service(exit_cfg))
        .serve(
            svc::PORT,
            crate::ports_sharing::port_service(wired.members.clone(), serve.ports.clone()),
        )
        .serve(
            svc::INVITE,
            crate::invite::server::invite_service(
                cfg.network_key.clone(),
                domain_list_cell.clone(),
                wired.members.access(),
                cfg.net_kind,
                cfg.reply_secret.clone(),
            ),
        )
}

/// The sync services, the order and discovery sources, the `found` tap — and building the stack.
fn with_discovery(
    builder: NetStackBuilder,
    (sources, found_tx): Finding,
    sync_svcs: SyncServices,
) -> Result<NetStack> {
    let builder = sync_svcs
        .into_iter()
        .fold(builder, |b, (id, handler)| b.serve(id, handler));
    let order: Vec<&'static str> = sources.iter().map(|d| d.name()).collect();
    sources
        .into_iter()
        .fold(builder, |b, d| b.discovery(d))
        .discovery_priority(order)
        .found_tap(found_tx)
        .build()
        .map_err(NetError::into_anyhow)
}
