//! Choosing the destination address: an IP literal from the request frame or an answer of the resolver.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use super::address_guard::{first_allowed, is_forbidden, is_own_address};
use super::refusal::Refusal;

/// An IP literal from the frame: a closed one is refused. IPv4-mapped IPv6 is judged and opened as plain IPv4;
/// a public IPv6 literal is opened as is (a host without IPv6 connectivity fails later with a connect error).
pub(super) fn literal_address(
    ip: IpAddr,
    target_host: &str,
    port: u16,
) -> std::result::Result<SocketAddr, Refusal> {
    let ip = ip.to_canonical();
    if is_forbidden(ip) {
        return Err(Refusal::forbidden(target_host));
    }
    Ok(SocketAddr::new(ip, port))
}

/// Addresses of a name from DoH: the first open one; an empty answer means "no address", all closed means "closed".
pub(super) fn first_allowed_resolved_address(
    addrs: &[Ipv4Addr],
    target_host: &str,
    port: u16,
) -> std::result::Result<SocketAddr, Refusal> {
    if addrs.is_empty() {
        return Err(Refusal::no_address(format!(
            "{target_host} — no usable address"
        )));
    }
    first_allowed(addrs)
        .map(|ip| SocketAddr::from((ip, port)))
        .ok_or_else(|| Refusal::forbidden(target_host))
}

/// The node's own address — the same owner services as the loopback. Checked before connecting: a refusal
/// doesn't reveal whether the port is listening.
pub(super) fn reject_own_address(addr: SocketAddr, target_host: &str) -> std::result::Result<SocketAddr, Refusal> {
    if is_own_address(addr.ip()) {
        return Err(Refusal::forbidden(target_host));
    }
    Ok(addr)
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
