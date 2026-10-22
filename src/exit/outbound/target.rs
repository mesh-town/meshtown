//! Choosing the destination addresses: an IP literal from the request frame or the answers of the resolver.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio::time::timeout;

use super::address_guard::{is_forbidden, is_own_address};
use super::refusal::Refusal;
use crate::doh::Resolver;

/// The most addresses of one family tried for a name.
pub(in crate::exit) const MAX_CANDIDATES: usize = 4;
/// How long the exit waits for an AAAA answer at most.
pub(in crate::exit) const AAAA_LOOKUP_TIMEOUT: Duration = Duration::from_secs(3);

/// What a resolver answered for one family: the open addresses in answer order, whether the answer had any address
/// at all (closed ones included), and whether the name does not exist.
#[derive(Debug)]
pub(in crate::exit) struct NameAnswer {
    pub open: Vec<SocketAddr>,
    pub any: bool,
    pub nx: bool,
}

impl NameAnswer {
    fn of(ips: Vec<IpAddr>, port: u16, nx: bool) -> NameAnswer {
        NameAnswer {
            any: !ips.is_empty(),
            open: allowed_addresses(ips, port),
            nx,
        }
    }

    fn empty() -> NameAnswer {
        NameAnswer::of(Vec::new(), 0, false)
    }
}

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

/// The addresses of an answer worth connecting to: each one in its canonical form (an IPv4-mapped IPv6 address is
/// judged and used as IPv4), closed and the node's own ones dropped, answer order kept, at most `MAX_CANDIDATES`.
pub(in crate::exit) fn allowed_addresses(ips: impl IntoIterator<Item = IpAddr>, port: u16) -> Vec<SocketAddr> {
    ips.into_iter()
        .map(|ip| ip.to_canonical())
        .filter(|ip| !is_forbidden(*ip) && !is_own_address(*ip))
        .take(MAX_CANDIDATES)
        .map(|ip| SocketAddr::new(ip, port))
        .collect()
}

/// The IPv4 answer for a name. A DoH error is "no address" right away: a second query would only double the wait.
pub(in crate::exit) async fn v4_answer(
    resolver: &Resolver,
    name: &str,
    port: u16,
) -> std::result::Result<NameAnswer, Refusal> {
    let (addrs, nx) = resolver
        .resolve_cached_nx(name)
        .await
        .map_err(|e| Refusal::no_address(format!("{name} — DNS: {e:#}")))?;
    Ok(NameAnswer::of(addrs.into_iter().map(IpAddr::V4).collect(), port, nx))
}

/// The IPv6 answer for a name within `wait`; an error or a timeout is an empty answer.
pub(in crate::exit) async fn v6_answer(resolver: &Resolver, name: &str, port: u16, wait: Duration) -> NameAnswer {
    match timeout(wait, resolver.resolve_cached_v6(name)).await {
        Ok(Ok(addrs)) => NameAnswer::of(addrs.into_iter().map(IpAddr::V6).collect(), port, false),
        _ => NameAnswer::empty(),
    }
}

/// Where a UDP datagram for a name goes: the first open IPv4 address, otherwise the first open IPv6 one. No
/// retries; a DoH error, a name that does not exist and an all-closed answer give `None`.
pub(in crate::exit) async fn udp_address(resolver: &Resolver, name: &str, port: u16) -> Option<SocketAddr> {
    let v4 = v4_answer(resolver, name, port).await.ok()?;
    if let Some(first) = v4.open.first() {
        return Some(*first);
    }
    if v4.nx {
        return None;
    }
    v6_answer(resolver, name, port, AAAA_LOOKUP_TIMEOUT).await.open.first().copied()
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
