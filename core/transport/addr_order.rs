//! Order of a node's direct addresses before they are truncated to a few.
//!
//! What it does: finds the source addresses of the default route (`PrimaryIps::probe`) and
//! puts them first, then public ones, then the rest (VM bridges, docker, etc.),
//! and a foreign tunnel address 100.64/10 (Tailscale, CGNAT) last. Truncation to the cap
//! (`truncate_ranked`) cuts the tunnel first, but keeps it if there is neither a route
//! nor a public address without it.
//! What it does not do: it does not remove addresses and does not touch the network (UDP `connect` sends no packets).
//! Links: called by `net_mainline::record::entry_from_addr`, `invite::make`, `net_mainline::cycle`.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};

use crate::doh::is_fake_ip;

/// Global IPv4 target used to pick a route (no packet is sent).
const PROBE_V4: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53);
/// Global IPv6 target used to pick a route (no packet is sent).
const PROBE_V6: SocketAddr = SocketAddr::new(
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
    53,
);

/// Source addresses of the default route, per address family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrimaryIps {
    pub v4: Option<Ipv4Addr>,
    pub v6: Option<Ipv6Addr>,
}

impl PrimaryIps {
    /// Nothing is known about the route.
    pub const NONE: PrimaryIps = PrimaryIps { v4: None, v6: None };

    /// Asks the OS about the default route; a family with no route yields `None`.
    pub fn probe() -> PrimaryIps {
        let v4 = route_source(SocketAddr::from(([0, 0, 0, 0], 0)), PROBE_V4);
        let v6 = route_source(SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)), PROBE_V6);
        PrimaryIps {
            v4: v4.and_then(|ip| match ip {
                IpAddr::V4(a) => Some(a),
                IpAddr::V6(_) => None,
            }),
            v6: v6.and_then(|ip| match ip {
                IpAddr::V6(a) => Some(a),
                IpAddr::V4(_) => None,
            }),
        }
    }

    /// Returns whether the address matches the route address of its own family.
    fn is_primary(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(a) => self.v4 == Some(a),
            IpAddr::V6(a) => self.v6 == Some(a),
        }
    }
}

/// Returns the source address when sending to `target`: `bind` → `connect` → `local_addr`.
fn route_source(bind: SocketAddr, target: SocketAddr) -> Option<IpAddr> {
    let sock = UdpSocket::bind(bind).ok()?;
    sock.connect(target).ok()?;
    let ip = sock.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}

/// Returns whether the address is public: not loopback, not private, not link-local, not CGNAT, not fake-ip.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => is_public_v4(a),
        IpAddr::V6(a) => is_public_v6(a),
    }
}

/// Address from 100.64.0.0/10 (RFC 6598): a Tailscale tunnel or an ISP's CGNAT. The address
/// is first normalized to canonical form, so `::ffff:100.64.x.x` counts too.
pub fn is_foreign_tunnel(ip: IpAddr) -> bool {
    matches!(ip.to_canonical(), IpAddr::V4(a) if is_cgnat_v4(a))
}

fn is_cgnat_v4(a: Ipv4Addr) -> bool {
    let o = a.octets();
    o[0] == 100 && (o[1] & 0xc0) == 64
}

fn is_public_v4(a: Ipv4Addr) -> bool {
    let cgnat = is_cgnat_v4(a);
    let special = a.is_loopback()
        || a.is_unspecified()
        || a.is_private()
        || a.is_link_local()
        || a.is_broadcast()
        || a.is_multicast();
    !(special || cgnat || is_fake_ip(a))
}

fn is_public_v6(a: Ipv6Addr) -> bool {
    let first = a.segments()[0];
    let ula = (first & 0xfe00) == 0xfc00;
    let link_local = (first & 0xffc0) == 0xfe80;
    !(a.is_loopback() || a.is_unspecified() || a.is_multicast() || ula || link_local)
}

/// Rank: default-route address.
const RANK_ROUTE: u8 = 0;
/// Rank: public address.
const RANK_PUBLIC: u8 = 1;
/// Rank: other address (VM bridges, docker, private network).
const RANK_OTHER: u8 = 2;
/// Rank: foreign tunnel 100.64/10.
const RANK_TUNNEL: u8 = 3;

/// Rank of an address: default route, public, other, foreign tunnel (100.64/10; it is
/// last even if it is the default-route address).
fn rank(addr: &SocketAddr, primary: &PrimaryIps) -> u8 {
    let ip = addr.ip();
    if is_foreign_tunnel(ip) {
        return RANK_TUNNEL;
    }
    if primary.is_primary(ip) {
        return RANK_ROUTE;
    }
    if is_public(ip) {
        return RANK_PUBLIC;
    }
    RANK_OTHER
}

/// Stably orders addresses by `(rank, family)`: IPv4 before IPv6 within a rank.
pub fn order_direct(addrs: &mut [SocketAddr], primary: &PrimaryIps) {
    addrs.sort_by_key(|a| (rank(a, primary), a.is_ipv6()));
}

/// Truncates the ordered (`order_direct`) list to `max` addresses. If none of the remaining
/// addresses is a route address, a public one or a tunnel, and a tunnel address was cut, the last slot
/// goes to the best tunnel address: for a "tailnet only" pair it is the only reachable one.
pub fn truncate_ranked(addrs: &mut Vec<SocketAddr>, max: usize, primary: &PrimaryIps) {
    let kept = tunnel_for_last_slot(addrs, max, primary);
    addrs.truncate(max);
    kept.into_iter().for_each(|t| addrs[max - 1] = t);
}

/// Tunnel address that must take the last slot after truncation to `max`, if one is needed.
fn tunnel_for_last_slot(
    addrs: &[SocketAddr],
    max: usize,
    primary: &PrimaryIps,
) -> Option<SocketAddr> {
    if addrs.len() <= max || max == 0 {
        return None;
    }
    let covered = addrs[..max].iter().any(|a| rank(a, primary) != RANK_OTHER);
    addrs[max..]
        .iter()
        .copied()
        .find(|a| !covered && rank(a, primary) == RANK_TUNNEL)
}

#[cfg(test)]
#[path = "addr_order_tests.rs"]
mod tests;
