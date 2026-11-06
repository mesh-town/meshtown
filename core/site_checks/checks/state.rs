//! The `Checks` type (replica, resolver, schedule) and its constructor with live
//! handles (turn on/off, change the interval, mark a domain "being checked now").
//! For publishing and reading the document's results, see `store.rs`; for the schedule, `schedule.rs`.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::exit::PeerConnPool;
use crate::members::membership::Members;
use crate::proxy::domains::DomainList;
use crate::site_checks::reachability::CheckConfig;
use crate::transport::net::{NetStack, PeerId, Replica};

use super::exits::ExitResults;
use super::schedule::{DomainStates, InProgress, Schedule};

/// The shared resolver: created once, shared between tasks.
mod doh_resolver {
    use std::sync::Arc;

    pub type Shared = Arc<crate::site_checks::doh::Resolver>;
}

/// A node's checks: the replica, the resolver and the schedule.
#[derive(Clone)]
pub struct Checks {
    pub(super) domain_list: DomainList,
    pub(super) shared_replica: Arc<dyn Replica>,
    pub(super) check_config: Arc<CheckConfig>,
    pub(super) schedule: Schedule,
    pub(super) resolver: doh_resolver::Shared,
    pub(super) domain_states: Arc<Mutex<DomainStates>>,
    pub(super) printed_check_keys: Arc<Mutex<HashSet<(PeerId, String, u64)>>>,
    pub(super) members: Members,
    pub(super) stack: NetStack,
    pub(super) pool: PeerConnPool,
    pub(super) exit_results: Arc<std::sync::Mutex<ExitResults>>,
    enabled: Arc<AtomicBool>,
    in_progress: Arc<InProgress>,
}

impl Checks {
    /// Assembles the checks on top of an opened domain list; `exit_access` is what checks through members' exits
    /// need: the network stack, a connection pool for the exit service and the list of online members.
    pub fn new(
        domain_list: DomainList,
        resolver: Arc<crate::site_checks::doh::Resolver>,
        check_config: CheckConfig,
        schedule: Schedule,
        enabled: bool,
        exit_access: (NetStack, PeerConnPool, Members),
    ) -> Checks {
        let (stack, pool, members) = exit_access;
        Checks {
            shared_replica: domain_list.shared_replica(),
            domain_list,
            check_config: Arc::new(check_config),
            schedule,
            resolver,
            domain_states: Arc::new(Mutex::new(DomainStates::default())),
            printed_check_keys: Arc::new(Mutex::new(HashSet::new())),
            members,
            stack,
            pool,
            exit_results: Arc::default(),
            enabled: Arc::new(AtomicBool::new(enabled)),
            in_progress: Arc::new(InProgress::default()),
        }
    }

    /// Whether checks are enabled right now.
    pub(super) fn checks_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Live handle: enables or disables the scheduled checks without restarting the task.
    pub(crate) fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    /// Live handle: changes the base interval of the scheduled loop.
    pub(crate) fn set_interval(&self, d: Duration) {
        self.schedule.set_base_interval(d);
    }

    /// Marks a domain "being checked now"; `false` means it is already being checked (by the scheduled loop or
    /// by another manual call).
    pub fn try_begin_check(&self, domain: &str) -> bool {
        let domain =
            crate::proxy::domains::normalize_domain(domain).unwrap_or_else(|_| domain.to_string());
        self.in_progress.try_begin(&domain)
    }

    /// Clears the "being checked" mark from a domain.
    pub fn end_check(&self, domain: &str) {
        let domain =
            crate::proxy::domains::normalize_domain(domain).unwrap_or_else(|_| domain.to_string());
        self.in_progress.end(&domain);
    }

    /// A snapshot of the set of domains being checked right now.
    pub fn domains_in_progress(&self) -> HashSet<String> {
        self.in_progress.domains_in_progress()
    }

    /// The DoH server address — for the start line.
    pub fn doh_server(&self) -> SocketAddr {
        self.resolver.server()
    }
}
