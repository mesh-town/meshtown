//! The third start phase: the domain list, checks, path choice and the proxy configuration — everything that is set up
//! to receive traffic.

use std::collections::HashSet;
use std::sync::Arc;

use crate::transport::net::{NetError, REPLICA_DOMAINS, REPLICA_NET};
use anyhow::Result;

use crate::invite::store::Ledger;
use crate::members::access::runtime::Access;
use crate::node::node_name;
use crate::node::node_state;
use crate::node::output::say;
use crate::node::program_version;
use crate::ports_sharing::announce::run_announce;
use crate::ports_sharing::open::Opener;
use crate::ports_sharing::PortsDoc;
use crate::proxy::domains::{self, DomainList};
use crate::proxy::route::RouteCtx;
use crate::proxy::ProxyConfig;
use crate::site_checks::checks::{self, Checks, Schedule};
use crate::site_checks::geoip;

use crate::node::config::NodeConfig;
use crate::node::DocChanges;

use super::access_wiring::{start_access, AccessStart};
use super::boot::Boot;
use super::network::Net;
use super::proxy_wiring::proxy_config_of;
use super::transports_relay::wire_relay;

/// The domain list, checks, path choice and the proxy configuration — everything that is set up to receive traffic.
pub(super) struct Serving {
    pub(super) domain_list: DomainList,
    pub(super) access: Arc<Access>,
    pub(super) checks: Checks,
    pub(super) route: RouteCtx,
    /// What a proxy front serves this node's traffic with.
    pub(super) proxy: ProxyConfig,
    pub(super) ports_doc: PortsDoc,
    pub(super) doc_changes: Arc<DocChanges>,
    pub(super) opener: Opener,
}

/// Background announcements in the network document: the program version and the node name (publisher and watcher).
fn spawn_announcements(boot: &mut Boot, domain_list: &DomainList, net: &Net) {
    boot.background_abort_handles
        .push(program_version::spawn_program_version_announce(domain_list));
    boot.background_abort_handles
        .extend(node_name::spawn_node_name(domain_list, &net.ctx));
    boot.background_abort_handles
        .push(crate::invite::store::spawn_sweeper(Ledger::of(domain_list)));
}

/// Wires relay through members on the `hyper` transport, if there is one; the advert task joins the node's tasks.
fn wire_relay_in(cfg: &NodeConfig, boot: &mut Boot, net: &Net, domain_list: &DomainList) {
    let Some(hyper) = boot.hyper.clone() else {
        return;
    };
    let task = wire_relay(
        &hyper,
        cfg.settings.relay,
        &net.members,
        domain_list.shared_replica(),
    );
    boot.background_abort_handles.push(task.abort_handle());
}

/// The node's country poll goes through the first transport (the list is non-empty after `check_transport_names`).
fn spawn_country_check(cfg: &NodeConfig, boot: &mut Boot, domain_list: &DomainList) {
    let Some(transport) = boot.transports.first().filter(|_| cfg.geoip) else {
        return;
    };
    let task = geoip::spawn_country_check(transport.clone(), domain_list);
    boot.background_abort_handles.push(task);
}

/// Opens the domain list, brings up the checks and builds the proxy configuration.
pub(super) async fn setup_serving(
    cfg: &NodeConfig,
    boot: &mut Boot,
    net: &Net,
    access: AccessStart,
) -> Result<Serving> {
    let (domain_list, doc_changes, access) =
        open_domain_list(net, access, &mut boot.background_abort_handles).await?;
    spawn_announcements(boot, &domain_list, net);
    wire_relay_in(cfg, boot, net, &domain_list);
    spawn_country_check(cfg, boot, &domain_list);

    let checks = setup_checks(cfg, net, &domain_list, &mut boot.background_abort_handles).await?;

    let route = RouteCtx::with_shared_exits(
        net.members.clone(),
        domain_list.clone(),
        boot.id,
        net.route_exits.clone(),
    );
    let proxy = proxy_config_of(&route, net);
    log_exit_and_blocked(cfg, net);

    let (ports_doc, opener) = setup_ports(cfg, boot, net, &domain_list);

    Ok(Serving {
        domain_list,
        access,
        checks,
        route,
        proxy,
        ports_doc,
        doc_changes,
        opener,
    })
}

/// Port publication: the document, the announcer in the node's tasks and the opener of others' ports from the config.
fn setup_ports(
    cfg: &NodeConfig,
    boot: &mut Boot,
    net: &Net,
    domain_list: &DomainList,
) -> (PortsDoc, Opener) {
    let ports_doc = PortsDoc::new(domain_list.shared_replica());
    boot.background_abort_handles.push(
        crate::node::output::spawn(run_announce(ports_doc.clone(), net.ports.clone()))
            .abort_handle(),
    );
    let opener = Opener::new(net.stack.clone(), net.members.clone());
    opener.reconcile_listeners(cfg.settings.ports_opened.clone());
    (ports_doc, opener)
}

/// Opens the domain list and its replicas, then starts access and the background tasks.
async fn open_domain_list(
    net: &Net,
    access: AccessStart,
    background_abort_handles: &mut Vec<tokio::task::AbortHandle>,
) -> Result<(DomainList, Arc<DocChanges>, Arc<Access>)> {
    let sync = net.sync_backend.as_ref();
    let doc_changes = Arc::new(DocChanges::default());
    let shared = sync
        .open_replica(REPLICA_NET)
        .await
        .map_err(NetError::into_anyhow)?;
    let own = sync
        .open_replica(REPLICA_DOMAINS)
        .await
        .map_err(NetError::into_anyhow)?;
    let domain_list = DomainList::open(shared, own).await?;
    say(
        "docs",
        format!(
            "domain list: space {}, author {}",
            domain_list.shared_replica_id().fmt_short(),
            domain_list.me().fmt_short()
        ),
    );
    let access = start_domain_background(
        net,
        &domain_list,
        &doc_changes,
        access,
        background_abort_handles,
    )
    .await;
    Ok((domain_list, doc_changes, access))
}

/// Starts access on the opened list, publishes it to the cell and starts the background tasks
/// (the event log, the exit flag announcer).
async fn start_domain_background(
    net: &Net,
    domain_list: &DomainList,
    doc_changes: &Arc<DocChanges>,
    access: AccessStart,
    background_abort_handles: &mut Vec<tokio::task::AbortHandle>,
) -> Arc<Access> {
    let access = start_access(domain_list, &net.members, access, background_abort_handles).await;
    let _ = net.domain_list_cell.set(domain_list.clone());
    let events = domains::log_events(domain_list.clone(), doc_changes.clone());
    background_abort_handles.push(crate::node::output::spawn(events).abort_handle());
    background_abort_handles.push(
        crate::node::output::spawn(node_state::run_exit_announce(
            domain_list.shared_replica(),
            net.exit_on.clone(),
            net.exit_announce_wakeup.clone(),
        ))
        .abort_handle(),
    );
    access
}

/// Builds `Checks`, starts their background task and adds the initial domains (the task
/// always starts, whether it is enabled is checked on every tick inside it).
async fn setup_checks(
    cfg: &NodeConfig,
    net: &Net,
    domain_list: &DomainList,
    background_abort_handles: &mut Vec<tokio::task::AbortHandle>,
) -> Result<Checks> {
    let schedule = Schedule::with_base_interval(cfg.settings.check_interval());
    let checks = Checks::new(
        domain_list.clone(),
        net.resolver.clone(),
        net.check_cfg.clone(),
        schedule,
        cfg.settings.checks_enabled,
    );
    // The task always starts: whether it is enabled is checked on every tick inside it.
    background_abort_handles
        .push(crate::node::output::spawn(checks::run_checks(checks.clone())).abort_handle());
    add_typed_domains(cfg, domain_list).await?;
    Ok(checks)
}

/// Adds the domains given at start to the list and writes a line for each.
async fn add_typed_domains(cfg: &NodeConfig, domain_list: &DomainList) -> Result<()> {
    for d in &cfg.initial_domains {
        let d = domain_list.add(d).await?;
        say("domains", format!("+ {d} (added locally)"));
    }
    Ok(())
}

/// Logs exit work and the list of blocked domains.
fn log_exit_and_blocked(cfg: &NodeConfig, net: &Net) {
    say("exit", exit_line(cfg.settings.exit_enabled));
    blocked_line(&net.blocked_domains)
        .into_iter()
        .for_each(|line| say("route", line));
}

/// A log line about exit work.
fn exit_line(exit_enabled: bool) -> String {
    match exit_enabled {
        true => format!(
            "serving as exit for members of network {}",
            crate::node::output::NET_NAME
        ),
        false => "not serving as exit (--no-exit)".to_string(),
    }
}

/// A log line about blocked domains (sorted); with none — `None`.
fn blocked_line(blocked_domains: &HashSet<String>) -> Option<String> {
    if blocked_domains.is_empty() {
        return None;
    }
    let mut names: Vec<&str> = blocked_domains.iter().map(|d| d.as_str()).collect();
    names.sort_unstable();
    Some(format!("treating as blocked: {}", names.join(", ")))
}

#[cfg(test)]
#[path = "serving_tests.rs"]
mod tests;
