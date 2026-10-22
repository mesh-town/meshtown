//! Admission by receiving interface: the machine's interface view and the pure `judge` verdict (no network and no OS).

use std::net::{IpAddr, Ipv4Addr};

use super::iface_kind::is_router;
use crate::config_file::{IfaceFilter, ListenAccess};

/// One IPv4 address of an interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ViewAddr {
    pub name: String,
    pub ip: Ipv4Addr,
    pub prefix: u8,
    pub broadcast: bool,
}

/// The machine's interface view (loopback is not included).
#[derive(Clone, Debug, Default)]
pub(crate) struct IfaceView {
    pub addrs: Vec<ViewAddr>,
    pub default_ip: Option<Ipv4Addr>,
    pub forwarding: bool,
}

impl IfaceView {
    /// The interface of the default-route address.
    pub(crate) fn default_iface(&self) -> Option<&str> {
        self.name_of(self.default_ip?)
    }

    /// The interface that owns the address.
    pub(crate) fn name_of(&self, ip: Ipv4Addr) -> Option<&str> {
        let found = self.addrs.iter().find(|a| a.ip == ip);
        found.map(|a| a.name.as_str())
    }
}

/// Who decided: automatic, list (`Only` and `Any`), `localhost` mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum By {
    Auto,
    List,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    Loopback,
    SelfHost,
    Localhost,
    Any,
    Listed,
    NotListed,
    AutoWan,
    Open,
}

impl Why {
    /// Admission, who decided, and the reason text for the log. The only `match` on `Why`.
    pub(crate) fn meta(self) -> (bool, By, &'static str) {
        match self {
            Why::Loopback => (true, By::None, "connection from this same machine"),
            Why::SelfHost => (true, By::None, "the machine's own address"),
            Why::Localhost => (false, By::None, "\"this computer only\" mode"),
            Why::Any => (true, By::List, "list \"*\" — no restrictions"),
            Why::Listed => (true, By::List, "the interface is in the list"),
            Why::NotListed => (false, By::List, "the interface is not in the list"),
            Why::AutoWan => (false, By::Auto, "automatic: the router's ISP network is closed"),
            Why::Open => (true, By::Auto, "automatic: the network is open"),
        }
    }
}

pub(crate) struct Verdict {
    pub why: Why,
    pub iface: Option<String>,
}

impl Verdict {
    pub(crate) fn allows(&self) -> bool {
        self.why.meta().0
    }
}

fn in_subnet(a: &ViewAddr, ip: Ipv4Addr) -> bool {
    let shift = 32 - u32::from(a.prefix.min(32));
    let mask = u32::MAX.checked_shl(shift).unwrap_or(0);
    u32::from(a.ip) & mask == u32::from(ip) & mask
}

/// The entry interface: by the client's subnet, else by the receiving address, else `None`.
pub(crate) fn ingress(
    view: &IfaceView,
    peer: Ipv4Addr,
    local: Option<Ipv4Addr>,
) -> Option<&str> {
    let local_iface = local.and_then(|l| view.name_of(l));
    let hits: Vec<&ViewAddr> = view.addrs.iter().filter(|a| in_subnet(a, peer)).collect();
    let same = hits.iter().find(|a| Some(a.name.as_str()) == local_iface);
    match same.or(hits.first()) {
        Some(a) => Some(a.name.as_str()),
        None => local_iface,
    }
}

/// The verdict for the entry interface (without loopback and `self`).
pub(crate) fn judge_iface(
    mode: ListenAccess,
    filter: &IfaceFilter,
    view: &IfaceView,
    iface: Option<&str>,
) -> Why {
    if mode == ListenAccess::Localhost {
        return Why::Localhost;
    }
    match filter {
        IfaceFilter::Any => Why::Any,
        IfaceFilter::Only(names) => listed_or_not(names, iface),
        IfaceFilter::Auto => auto_verdict(view, iface),
    }
}

fn listed_or_not(names: &[String], iface: Option<&str>) -> Why {
    let listed = iface.is_some_and(|i| names.iter().any(|n| n == i));
    if listed {
        return Why::Listed;
    }
    Why::NotListed
}

fn auto_verdict(view: &IfaceView, iface: Option<&str>) -> Why {
    let wan = is_router(view) && iface.is_some() && iface == view.default_iface();
    if wan {
        return Why::AutoWan;
    }
    Why::Open
}

fn canonical_v4(ip: IpAddr) -> Option<Ipv4Addr> {
    match ip.to_canonical() {
        IpAddr::V4(v4) => Some(v4),
        IpAddr::V6(_) => None,
    }
}

fn verdict(why: Why, iface: Option<&str>) -> Verdict {
    Verdict { why, iface: iface.map(str::to_string) }
}

/// The full verdict for a connection.
pub(crate) fn judge(
    mode: ListenAccess,
    filter: &IfaceFilter,
    view: &IfaceView,
    peer: IpAddr,
    local: Option<IpAddr>,
) -> Verdict {
    if peer.to_canonical().is_loopback() {
        return verdict(Why::Loopback, None);
    }
    if mode == ListenAccess::Localhost {
        return verdict(Why::Localhost, None);
    }
    let Some(peer4) = canonical_v4(peer) else {
        return verdict(Why::NotListed, None);
    };
    if view.name_of(peer4).is_some() {
        return verdict(Why::SelfHost, None);
    }
    let local4 = local.and_then(canonical_v4);
    let iface = ingress(view, peer4, local4);
    verdict(judge_iface(mode, filter, view, iface), iface)
}

#[cfg(test)]
#[path = "iface_gate_tests.rs"]
mod tests;
