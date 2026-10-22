//! The third start phase: the domain list, checks, path choice and the SOCKS5 server — everything that is set up
//! to receive traffic.

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use crate::net::{NetError, REPLICA_DOMAINS, REPLICA_NET};
use anyhow::Result;
use tokio::net::TcpListener;

use crate::access::runtime::Access;
use crate::checks::{self, Checks, Schedule};
use crate::domains::{self, DomainList};
use crate::exit::PeerConnPool;
use crate::geoip;
use crate::invite::store::Ledger;
use crate::node_state;
use crate::output::say;
use crate::ports::announce::run_announce;
use crate::ports::open::Opener;
use crate::ports::PortsDoc;
use crate::node_name;
use crate::program_version;
use crate::proxy::{self, ProxyConfig};
use crate::route::RouteCtx;
use crate::web::auth::iface_nets::admitted_lan;
use crate::web::auth::iface_os::current_view;

use crate::config_file::ListenAccess;
use crate::node::config::NodeConfig;
use crate::node::socks_task::SocksTask;
use crate::node::DocChanges;

use super::access_wiring::{start_access, AccessStart};
use super::boot::Boot;
use super::network::Net;

/// The domain list, checks, path choice and the SOCKS5 server — everything that is set up to receive traffic.
pub(super) struct Serving {
    pub(super) domain_list: DomainList,
    pub(super) access: Arc<Access>,
    pub(super) checks: Checks,
    pub(super) route: RouteCtx,
    pub(super) socks_addr: Option<SocketAddr>,
    /// The SOCKS5 accept task; `shutdown` waits for it to end so the port is free on return.
    pub(super) socks_task: SocksTask,
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

/// The node's country poll goes through the first transport (the list is non-empty after `check_transport_names`).
fn spawn_country_check(cfg: &NodeConfig, boot: &mut Boot, domain_list: &DomainList) {
    let Some(transport) = boot.transports.first().filter(|_| cfg.geoip) else { return };
    let task = geoip::spawn_country_check(transport.clone(), domain_list);
    boot.background_abort_handles.push(task);
}

/// Opens the domain list, brings up the checks and the SOCKS5 server on the already taken port.
pub(super) async fn setup_serving(
    cfg: &NodeConfig,
    boot: &mut Boot,
    net: &Net,
    access: AccessStart,
) -> Result<Serving> {
    let (domain_list, doc_changes, access) =
        open_domain_list(net, access, &mut boot.background_abort_handles).await?;
    spawn_announcements(boot, &domain_list, net);
    spawn_country_check(cfg, boot, &domain_list);

    let checks = setup_checks(cfg, net, &domain_list, &mut boot.background_abort_handles).await?;

    let route = RouteCtx::with_shared_exits(
        net.members.clone(),
        domain_list.clone(),
        boot.id,
        net.route_exits.clone(),
    );
    let (socks_addr, socks_task) = spawn_socks_server(cfg, boot, net, &route)?
        .map_or((None, SocksTask::default()), |(addr, task)| (Some(addr), task));

    let (ports_doc, opener) = setup_ports(cfg, boot, net, &domain_list);

    Ok(Serving {
        domain_list,
        access,
        checks,
        route,
        socks_addr,
        socks_task,
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
        tokio::spawn(run_announce(ports_doc.clone(), net.ports.clone())).abort_handle(),
    );
    let opener = Opener::new(net.stack.clone(), net.members.clone());
    opener.reconcile_listeners(cfg.settings.ports_opened.clone());
    (ports_doc, opener)
}

/// Opens the domain list, fills the exit protocol cell and starts its background tasks
/// (the event log, the exit flag announcer).
async fn open_domain_list(
    net: &Net,
    access: AccessStart,
    background_abort_handles: &mut Vec<tokio::task::AbortHandle>,
) -> Result<(DomainList, Arc<DocChanges>, Arc<Access>)> {
    let sync = net.sync_backend.as_ref();
    let doc_changes = Arc::new(DocChanges::default());
    let shared = sync.open_replica(REPLICA_NET).await.map_err(NetError::into_anyhow)?;
    let own = sync.open_replica(REPLICA_DOMAINS).await.map_err(NetError::into_anyhow)?;
    let domain_list = DomainList::open(shared, own).await?;
    say(
        "docs",
        format!(
            "domain list: space {}, author {}",
            domain_list.shared_replica_id().fmt_short(),
            domain_list.me().fmt_short()
        ),
    );
    // The boot run goes before the node's first record in the shared replica: otherwise `is_legacy` fires falsely.
    let access = start_access(&domain_list, &net.members, access, background_abort_handles).await;
    let _ = net.domain_list_cell.set(domain_list.clone());
    let events = domains::log_events(domain_list.clone(), doc_changes.clone());
    background_abort_handles.push(tokio::spawn(events).abort_handle());
    background_abort_handles.push(
        tokio::spawn(node_state::run_exit_announce(
            domain_list.shared_replica(),
            net.exit_on.clone(),
            net.exit_announce_wakeup.clone(),
        ))
        .abort_handle(),
    );
    Ok((domain_list, doc_changes, access))
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
    background_abort_handles.push(tokio::spawn(checks::run_checks(checks.clone())).abort_handle());
    for d in &cfg.initial_domains {
        let d = domain_list.add(d).await?;
        say("domains", format!("+ {d} (added locally)"));
    }
    Ok(checks)
}

/// The log line about SOCKS5 being off: the startup flag or a setting.
fn socks_off_line(by_flag: bool) -> &'static str {
    match by_flag {
        true => "off by the --no-socks flag",
        false => "off in settings (socks.enabled: false)",
    }
}

/// Brings up the SOCKS5 server on the already taken port (or logs that it is off) and logs
/// exit work and the list of blocked domains.
fn spawn_socks_server(
    cfg: &NodeConfig,
    boot: &mut Boot,
    net: &Net,
    route: &RouteCtx,
) -> Result<Option<(SocketAddr, SocksTask)>> {
    let socks_off = socks_off_line(cfg.socks_off_by_flag);
    let socks = serve_socks(boot.socks_listener.take(), route, net, (socks_off, proxy::Admission { access: cfg.socks.access, ifaces: cfg.socks.ifaces.clone() }))?;
    say("exit", exit_line(cfg.settings.exit_enabled));
    blocked_line(&net.blocked_domains).into_iter().for_each(|line| say("route", line));
    Ok(socks)
}

/// The SOCKS5 server on the taken port; with no listener — the `socks_off` line and `None`.
fn serve_socks(
    listener: Option<TcpListener>,
    route: &RouteCtx,
    net: &Net,
    (socks_off, adm): (&str, proxy::Admission),
) -> Result<Option<(SocketAddr, SocksTask)>> {
    let Some(listener) = listener else {
        say("socks", socks_off);
        return Ok(None);
    };
    let bound = listener.local_addr()?;
    let proxy_cfg = ProxyConfig {
        route: route.clone(),
        stack: net.stack.clone(),
        pool: PeerConnPool::default().with_traffic(net.exit_traffic.clone()),
        resolver: net.resolver.clone(),
        host_overrides: net.exit_hosts.clone(),
        udp_slots: proxy::udp_slots(),
    };
    let (access, lan) = (adm.access, lan_line(&adm, bound.port()));
    let task = SocksTask::new(tokio::spawn(proxy::serve_proxy(listener, proxy_cfg, adm)));
    say("socks", format!("accepting connections on {bound}{}", access_note(access)));
    lan.into_iter().for_each(|line| say("socks", line));
    Ok(Some((SocketAddr::from((Ipv4Addr::LOCALHOST, bound.port())), task)))
}

/// A note for the log line: in local-network mode the proxy has no password.
fn access_note(access: ListenAccess) -> &'static str {
    match access {
        ListenAccess::Localhost => "",
        ListenAccess::Localnet => " (from the local network, no password)",
    }
}

/// A log line with addresses for the network's devices; only in `localnet` mode.
fn lan_line(adm: &proxy::Admission, port: u16) -> Option<String> {
    (adm.access == ListenAccess::Localnet).then(|| {
        let all = crate::web::auth::access::lan_addrs();
        let open = admitted_lan(adm.access, &adm.ifaces, &current_view(), all);
        let list: Vec<String> = open
            .into_iter()
            .map(|(iface, ip)| format!("{ip}:{port} ({iface})"))
            .collect();
        match list.is_empty() {
            true => "no local network address found".to_string(),
            false => format!("addresses for network devices: {}", list.join(", ")),
        }
    })
}

/// A log line about exit work.
fn exit_line(exit_enabled: bool) -> String {
    match exit_enabled {
        true => format!("serving as exit for members of network {}", crate::output::NET_NAME),
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
