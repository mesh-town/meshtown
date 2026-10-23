//! A running node (`RunningNode`) and its live handles: enabling the exit, intervals, shutdown.

use portable_atomic::AtomicU64;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::transport::net::NetStack;
use crate::transport::net::{PeerAddr, PeerId};
use tokio::sync::{mpsc, watch, Notify};

use crate::exit::{Clients, ExitTraffic};
use crate::members::membership::Members;
use crate::members::network_key::NetworkKey;
use crate::node::node_name::effective_node_name;
use crate::node::output::say;
use crate::node::telemetry::Telemetry;
use crate::ports_sharing::open::Opener;
use crate::ports_sharing::{Opened, PortsDoc, Published, PublishedPortsTable};
use crate::proxy::domains::DomainList;
use crate::proxy::route::RouteCtx;
use crate::site_checks::checks::Checks;

use super::identity::IdentityState;

/// A running node: holds everything needed to introduce new addresses and to shut down.
///
/// `Clone`: the node lives while at least one holder of a copy is alive (`NodeCtl` holds its own while the node
/// is up); only the owner in `run_node_with_ctl` calls `shutdown(self)` — a clone must not call it
/// at all.
#[derive(Clone)]
pub struct RunningNode {
    pub id: PeerId,
    pub stack: NetStack,
    pub members: Members,
    pub found_tx: mpsc::Sender<PeerAddr>,
    pub gossip_neighbor_count: Arc<AtomicUsize>,
    pub ok_probe_count: Arc<AtomicUsize>,
    pub domains: DomainList,
    pub checks: Checks,
    /// What a proxy front serves this node's traffic with.
    pub(in crate::node) proxy: crate::proxy::ProxyConfig,
    /// The published state of the proxy listener.
    pub(crate) proxy_face: watch::Receiver<crate::proxy::ProxyFace>,
    /// The switch for exit work: the console flips it without restarting the node.
    pub exit_on: Arc<AtomicBool>,
    /// The registry of exit clients: who is going online through this node right now; read by `collect`.
    pub exit_clients: Clients,
    /// The registry of exit traffic speed per peer; read by `collect`.
    pub exit_traffic: ExitTraffic,
    /// Path choice (direct or via the selected exit) — the same one the SOCKS5 server uses.
    pub route: RouteCtx,
    /// The node name (by default from the key or from the settings file); the current one is
    /// `current_name()`.
    pub name_at_start: String,
    /// the same channel as in `MemberContext.own_name_tx`.
    pub(crate) own_name_tx: watch::Sender<String>,
    /// Whether the node identity is saved between runs.
    pub identity: IdentityState,
    /// The network key the node was started with.
    pub network_key: NetworkKey,
    /// The network label from the invite or the settings file.
    pub network_label: Option<String>,
    /// The node's live state for the page (relay, NAT, DHT, RSS, members).
    pub telemetry: Arc<Telemetry>,
    /// A snapshot of the address cache at startup (id → `seen_unix`), never updated — members connected in this
    /// run are in telemetry.
    pub cached_peers: Arc<HashMap<PeerId, u64>>,
    /// The live volume-probe interval; the same `Arc` that sits in the `ProbeSettings`
    /// of all connections. Reachable from outside only through `set_probe_interval`.
    pub(super) probe_interval_secs: Arc<AtomicU64>,
    /// The alarm of the exit-flag announcer (`node_state::run_exit_announce`):
    /// `set_exit_enabled` calls `notify_one` so the announcement goes out without waiting for the timer.
    pub(super) exit_announce_wakeup: Arc<Notify>,
    /// The table of own port publications: read by the `PortServer`, the announcer and the page.
    pub ports: PublishedPortsTable,
    /// The port publication document (the `port/tcp/…` records of the shared network document).
    pub ports_doc: PortsDoc,
    /// Change counters of the shared document: the snapshot cache uses them to decide whether to re-read the document.
    pub doc_changes: Arc<crate::node::DocChanges>,
    /// The opener of others' ports: listeners on `127.0.0.1:<local>`.
    pub opener: Opener,
    /// The membership list: verdicts, member actions and the watcher's state.
    pub access: Arc<crate::members::access::runtime::Access>,
    /// Handles of background tasks started in `start_node()` (except those spawned by
    /// `run_node_loop` itself — they end when connections close). `shutdown` aborts them
    /// after the stack stops.
    pub(super) background_abort_handles: Arc<Vec<tokio::task::AbortHandle>>,
}

impl RunningNode {
    /// The configuration a proxy front serves this node's traffic with (the connection pool is shared).
    pub fn proxy_config(&self) -> crate::proxy::ProxyConfig {
        self.proxy.clone()
    }

    /// Introduces the address of a member found from outside (for example, by a test).
    pub async fn introduce(&self, addr: PeerAddr) {
        let _ = self.found_tx.send(addr).await;
    }

    /// A live handle: the new node name (`None` or invalid — the default name); handshakes pick up
    /// it right away, the publisher updates the network document. The same name as before — no noise.
    pub fn set_node_name(&self, configured: Option<&str>) {
        let name = effective_node_name(configured, self.id);
        self.own_name_tx.send_if_modified(|cur| {
            let changed = *cur != name;
            changed.then(|| *cur = name);
            changed
        });
    }

    /// The current node name (taking `set_node_name` into account).
    pub fn current_name(&self) -> String {
        self.own_name_tx.borrow().clone()
    }

    /// A live handle: turns exit work on or off without restarting the node.
    pub fn set_exit_enabled(&self, on: bool) {
        self.exit_on.store(on, Ordering::Relaxed);
        self.exit_announce_wakeup.notify_one();
    }

    /// A live handle: turns scheduled checks on or off without restarting the node.
    pub fn set_checks_enabled(&self, on: bool) {
        self.checks.set_enabled(on);
    }

    /// A live handle: changes the base interval of scheduled checks.
    pub fn set_check_interval(&self, d: Duration) {
        self.checks.set_interval(d);
    }

    /// A live handle: replaces the list of selected exits without restarting the node.
    pub fn set_route_exits(&self, exits: Vec<PeerId>) {
        self.route.set_chosen_exits(exits);
    }

    /// A live handle: replaces the list of own port publications; the announcer wakes up on its own.
    pub fn set_ports_published(&self, v: Vec<Published>) {
        self.ports.set_published(v);
    }

    /// A live handle: replaces the list of others' ports opened here (call inside the tokio runtime).
    pub fn set_ports_opened(&self, v: Vec<Opened>) {
        self.opener.reconcile_listeners(v);
    }

    /// A live handle: changes the volume-probe interval for all live and new connections.
    pub fn set_probe_interval(&self, d: Duration) {
        self.probe_interval_secs
            .store(d.as_secs().max(1), Ordering::Relaxed);
    }

    /// Stops the node: closes the port opener and the network stack (the transport
    /// itself enforces the time limit for closing), then aborts the background tasks.
    pub async fn shutdown(self) {
        self.opener.close_all();
        self.stack.close().await;
        for handle in self.background_abort_handles.iter() {
            handle.abort();
        }
        flush_replicas(&self.domains).await;
        say("node", "stopped");
    }

    /// Everything `invite::make` needs to build an invite for a live node. Called by both
    /// `run_cycles` (the first cycle, the console) and `NodeCtl::invite`.
    pub(crate) fn invite_source(&self) -> crate::invite::Source {
        crate::invite::Source {
            stack: self.stack.clone(),
            members: self.members.clone(),
            key: self.network_key.clone(),
            label: self.network_label.clone(),
            domains: self.domains.clone(),
        }
    }
}

/// Writes the delayed state of both replicas to disk; a failure is a warning and does not stop the shutdown.
async fn flush_replicas(domains: &crate::proxy::domains::DomainList) {
    for replica in [domains.shared_replica(), domains.own_replica()] {
        if let Err(e) = replica.flush().await {
            say(
                "node",
                format!("warning: saving a replica on shutdown failed: {e}"),
            );
        }
    }
}
