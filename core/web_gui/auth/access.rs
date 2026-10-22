//! Admission of a peer and of the node's address (IPv4 listener only).

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use crate::config::config_file::ListenAccess;

fn v4_of(ip: IpAddr) -> Option<Ipv4Addr> {
    match ip {
        IpAddr::V4(v4) => Some(v4),
        IpAddr::V6(v6) => v6.to_ipv4_mapped(),
    }
}

fn local_range(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

/// `localhost` — only `127.0.0.0/8`; `localnet` — also `10/8`, `172.16/12`, `192.168/16`,
/// `169.254/16`. CGNAT, public addresses and IPv6 without an embedded IPv4 — no.
pub(crate) fn is_client_allowed(mode: ListenAccess, ip: IpAddr) -> bool {
    let Some(v4) = v4_of(ip) else {
        return false;
    };
    if v4.is_loopback() {
        return true;
    }
    mode == ListenAccess::Localnet && local_range(v4)
}

pub(crate) fn bind_ip(mode: ListenAccess) -> Ipv4Addr {
    match mode {
        ListenAccess::Localhost => Ipv4Addr::LOCALHOST,
        ListenAccess::Localnet => Ipv4Addr::UNSPECIFIED,
    }
}

/// Local address of a UDP socket "connected" to `192.0.2.1:9`: sends no packets.
pub(crate) fn lan_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    lan_from(sock.local_addr().ok().map(|a| a.ip()))
}

/// Private IPv4 only, not loopback.
pub(crate) fn lan_from(ip: Option<IpAddr>) -> Option<Ipv4Addr> {
    let IpAddr::V4(v4) = ip? else {
        return None;
    };
    let usable = !v4.is_loopback() && is_client_allowed(ListenAccess::Localnet, IpAddr::V4(v4));
    usable.then_some(v4)
}

/// An interface address for the network's devices: the interface name and IPv4.
pub(crate) type IfaceAddr = (String, Ipv4Addr);

/// Pure selection: not loopback and passes `is_client_allowed(Localnet, ..)`; order — the default-route
/// address (`default`) first, the rest by interface name (then by address), without repeats.
pub(crate) fn lan_addrs_pick(all: Vec<IfaceAddr>, default: Option<Ipv4Addr>) -> Vec<IfaceAddr> {
    let mut kept: Vec<IfaceAddr> = all
        .into_iter()
        .filter(|(_, ip)| {
            !ip.is_loopback() && is_client_allowed(ListenAccess::Localnet, IpAddr::V4(*ip))
        })
        .collect();
    kept.sort_by(|a, b| {
        (a.1 != default.unwrap_or(Ipv4Addr::BROADCAST), &a.0, a.1).cmp(&(
            b.1 != default.unwrap_or(Ipv4Addr::BROADCAST),
            &b.0,
            b.1,
        ))
    });
    kept.dedup();
    kept
}

/// This machine's addresses for the network's other devices (the OS call is a thin wrapper over `lan_addrs_pick`).
pub(crate) fn lan_addrs() -> Vec<IfaceAddr> {
    let all = if_addrs::get_if_addrs().unwrap_or_default();
    let v4 = all.into_iter().filter_map(|i| match i.addr {
        if_addrs::IfAddr::V4(a) => Some((i.name, a.ip)),
        if_addrs::IfAddr::V6(_) => None,
    });
    lan_addrs_pick(v4.collect(), lan_ipv4())
}

#[cfg(test)]
#[path = "access_tests.rs"]
mod tests;
