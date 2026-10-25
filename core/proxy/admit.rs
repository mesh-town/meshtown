//! Admission of a connection to the proxy by the client address: the same rules as for the node page
//! (`web.access`), and a refusal log without the scanner flooding the ring.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use crate::config::config_file::{IfaceFilter, ListenAccess};
use crate::node::output::{say, NET_NAME};
use crate::web_gui::auth::access::is_client_allowed;
use crate::web_gui::auth::iface_gate::{judge, IfaceView, Verdict};

/// How long we stay quiet about repeated refusals to one address.
const DENY_LOG_PERIOD: Duration = Duration::from_secs(300);
/// Limit of remembered addresses: a scanner with thousands of addresses will not blow up memory.
const DENY_LOG_CAP: usize = 256;

/// Whether to admit the client from address `ip` in mode `mode`: loopback — always; `localnet` — also
/// private ranges; public and CGNAT — no.
pub(super) fn is_proxy_client_allowed(mode: ListenAccess, ip: IpAddr) -> bool {
    is_client_allowed(mode, ip)
}

/// Whom the proxy admits: the `socks.access` mode and the `socks.ifaces` interface filter.
#[derive(Clone, Debug)]
pub struct Admission {
    pub access: ListenAccess,
    pub ifaces: IfaceFilter,
}

/// Verdict on an accepted connection: the client `client_addr`, the accepting address `local` (if the OS named it).
pub(super) fn admit_conn(
    adm: &Admission,
    view: &IfaceView,
    client_addr: SocketAddr,
    local: Option<SocketAddr>,
) -> Verdict {
    judge(
        adm.access,
        &adm.ifaces,
        view,
        client_addr.ip(),
        local.map(|a| a.ip()),
    )
}

/// Refusal log line: address, interface, reason and how to change it.
pub(super) fn iface_deny_text(client_addr: SocketAddr, v: &Verdict, adm: &Admission) -> String {
    let iface = v.iface.as_deref().unwrap_or("unknown");
    let (_, _, why) = v.why.meta();
    let list = match adm.ifaces.to_state() {
        Some(names) => names.join(", "),
        None => "automatic".to_string(),
    };
    format!(
        "{client_addr}: refused, interface \"{iface}\": {why} (socks.ifaces: {list}); change: the {NET_NAME} page or `{} ifaces`",
        crate::cli::program_name::program_name()
    )
}

/// Refusal log: one line for the first refusal of an address per period.
#[derive(Default)]
pub(super) struct DenyLog(HashMap<IpAddr, Instant>);

impl DenyLog {
    /// `true` — it is time to write about a refusal for this address (the first in the period), and it remembers it.
    pub(super) fn remember_if_first_in_period(&mut self, ip: IpAddr, now: Instant) -> bool {
        self.drop_expired(now);
        if self.0.contains_key(&ip) || self.0.len() >= DENY_LOG_CAP {
            return false;
        }
        self.0.insert(ip, now);
        true
    }

    /// Forgets the addresses whose quiet period is over.
    fn drop_expired(&mut self, now: Instant) {
        self.0
            .retain(|_, at| now.duration_since(*at) < DENY_LOG_PERIOD);
    }

    /// Refusal by interface: the same per-address period as `log_address_deny`; `text` is the ready line.
    pub(super) fn log_iface_deny(&mut self, client_addr: SocketAddr, text: &str) {
        if self.remember_if_first_in_period(client_addr.ip(), Instant::now()) {
            say("socks", text);
        }
    }

    /// A closed connection writes the first refusal to the log; the caller closes the socket itself.
    pub(super) fn log_address_deny(&mut self, client_addr: SocketAddr) {
        if self.remember_if_first_in_period(client_addr.ip(), Instant::now()) {
            say("socks", format!("{client_addr}: refused, address outside the local network (proxy without a password)"));
        }
    }
}

#[cfg(test)]
#[path = "admit_tests.rs"]
mod tests;
