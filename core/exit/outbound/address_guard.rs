//! Filters destination addresses for the exit role: a network member must not reach the host's
//! local network (router web admin, network drives) or service addresses through the exit.
//! What it does: a single check over `IpAddr` — for an IP literal from the request frame and for the addresses of
//! a name from DoH. IPv4 uses a table of closed networks; for IPv6 only global unicast is open, forms
//! with an embedded IPv4 are judged by it. Separately, a "the node's own address" probe with a socket, without packets.
//! What it does not do: it does not touch `doh::is_unusable` — it is called by the direct proxy path, the reachability probe
//! and geoip, where the client goes on its own behalf; it does not look at the test `host_overrides` table.
//! Links: called by `super::dial` and `super::target`.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, UdpSocket};

/// Closed IPv4 networks: network address and prefix length (1..=32).
const CLOSED_V4: [(Ipv4Addr, u8); 14] = [
    (Ipv4Addr::new(0, 0, 0, 0), 8),       // "this host"
    (Ipv4Addr::new(10, 0, 0, 0), 8),      // private network
    (Ipv4Addr::new(100, 64, 0, 0), 10),   // ISP CGNAT
    (Ipv4Addr::new(127, 0, 0, 0), 8),     // loopback
    (Ipv4Addr::new(169, 254, 0, 0), 16),  // link-local
    (Ipv4Addr::new(172, 16, 0, 0), 12),   // private network
    (Ipv4Addr::new(192, 0, 0, 0), 24),    // IETF service addresses
    (Ipv4Addr::new(192, 0, 2, 0), 24),    // documentation TEST-NET-1
    (Ipv4Addr::new(192, 168, 0, 0), 16),  // private network
    (Ipv4Addr::new(198, 18, 0, 0), 15),   // test benches and own fake-ip
    (Ipv4Addr::new(198, 51, 100, 0), 24), // documentation TEST-NET-2
    (Ipv4Addr::new(203, 0, 113, 0), 24),  // documentation TEST-NET-3
    (Ipv4Addr::new(224, 0, 0, 0), 4),     // multicast
    (Ipv4Addr::new(240, 0, 0, 0), 4),     // reserved and broadcast 255.255.255.255
];

/// IPv6 global unicast (2000::/3): everything outside it is closed.
const GLOBAL_V6: (Ipv6Addr, u8) = (Ipv6Addr::new(0x2000, 0, 0, 0, 0, 0, 0, 0), 3);

/// Closed networks inside 2000::/3: Teredo and 6to4 embed IPv4, the rest is documentation.
const CLOSED_V6: [(Ipv6Addr, u8); 4] = [
    (Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 0), 32), // Teredo
    (Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0), 32), // documentation
    (Ipv6Addr::new(0x2002, 0, 0, 0, 0, 0, 0, 0), 16), // 6to4
    (Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0), 20), // documentation
];

/// Whether the address is closed for the exit.
pub(in crate::exit) fn is_forbidden(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_v4(v4),
        IpAddr::V6(v6) => is_forbidden_v6(v6),
    }
}

fn is_forbidden_v4(ip: Ipv4Addr) -> bool {
    CLOSED_V4.iter().any(|&(net, len)| in_v4(ip, net, len))
}

/// `::a.b.c.d` and `::ffff:a.b.c.d` are judged by the embedded IPv4 (`::` and `::1` fall into 0/8).
fn is_forbidden_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4() {
        return is_forbidden_v4(v4);
    }
    let (global, global_len) = GLOBAL_V6;
    !in_v6(ip, global, global_len) || CLOSED_V6.iter().any(|&(net, len)| in_v6(ip, net, len))
}

/// A deliberately foreign address (TEST-NET-1): if binding to it succeeded, the kernel allows binding to foreign
/// addresses (Linux `ip_nonlocal_bind`), and the bind probe can't be trusted.
const FOREIGN: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));

/// The node's own address (the router's public WAN, a VPS address): a socket binds to its own address, and
/// the route to it picks it as the outgoing one; or an IPv6 neighbour in the exit's /64. Sends no packets; a probe
/// error means "not own".
pub(in crate::exit) fn is_own_address(ip: IpAddr) -> bool {
    if let IpAddr::V6(v6) = ip {
        return own_or_local(ip) || on_exit_link_v6(v6);
    }
    own_or_local(ip)
}

fn own_or_local(ip: IpAddr) -> bool {
    (binds_to(ip) && !binds_to(FOREIGN)) || routes_to_self(ip)
}

/// Whether the first 64 bits of two IPv6 addresses are equal.
fn same_v6_64(a: Ipv6Addr, b: Ipv6Addr) -> bool {
    u128::from(a) >> 64 == u128::from(b) >> 64
}

/// Whether the address shares a /64 with the source address the kernel picks for reaching it. An IPv4-mapped
/// address is never a neighbour; a socket or route failure means "no" (such a request cannot be delivered anyway).
fn on_exit_link_v6(ip: Ipv6Addr) -> bool {
    if ip.to_ipv4_mapped().is_some() {
        return false;
    }
    let source = UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0)).and_then(|sock| {
        sock.connect((ip, 9))?;
        sock.local_addr()
    });
    match source {
        Ok(local) => matches!(local.ip(), IpAddr::V6(src) if same_v6_64(src, ip)),
        Err(_) => false,
    }
}

fn binds_to(ip: IpAddr) -> bool {
    UdpSocket::bind((ip, 0)).is_ok()
}

/// UDP `connect` only picks a route and an outgoing address (Linux — the `local` table).
fn routes_to_self(ip: IpAddr) -> bool {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|sock| {
            sock.connect((ip, 9))?;
            sock.local_addr()
        })
        .is_ok_and(|local| local.ip() == ip)
}

fn in_v4(ip: Ipv4Addr, net: Ipv4Addr, len: u8) -> bool {
    let shift = 32 - u32::from(len);
    u32::from(ip) >> shift == u32::from(net) >> shift
}

fn in_v6(ip: Ipv6Addr, net: Ipv6Addr, len: u8) -> bool {
    let shift = 128 - u32::from(len);
    u128::from(ip) >> shift == u128::from(net) >> shift
}

#[cfg(test)]
#[path = "address_guard_link_tests.rs"]
mod link_tests;
#[cfg(test)]
#[path = "address_guard_tests.rs"]
mod tests;
