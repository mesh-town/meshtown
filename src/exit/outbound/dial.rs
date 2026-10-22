//! From a name or an IP literal to an open TCP stream: address choice, own-address check, connection.

use std::net::{IpAddr, SocketAddr};

use tokio::net::TcpStream;
use tokio::time::timeout;

use super::refusal::Refusal;
use super::target::{first_allowed_resolved_address, literal_address, reject_own_address};
use crate::exit::frame::{EXIT_CODE_CONNECT_FAILED, EXIT_TCP_CONNECT_TIMEOUT};
use crate::exit::server::ExitConfig;

/// Destination address: the test table as is; an IP literal goes through the filter (the client
/// sends the address in the request frame as a name); otherwise the resolver cache, on a miss
/// DoH, and the same filter on every address of the answer. The chosen address must not be the node's own.
pub(in crate::exit) async fn resolve_address(
    cfg: &ExitConfig,
    target_host: &str,
    port: u16,
) -> std::result::Result<SocketAddr, Refusal> {
    if let Some(addr) = cfg.host_overrides.get(target_host) {
        return Ok(*addr);
    }
    if let Ok(ip) = target_host.parse::<IpAddr>() {
        return literal_address(ip, target_host, port).and_then(|addr| reject_own_address(addr, target_host));
    }
    let addrs = cfg
        .resolver
        .resolve_cached(target_host)
        .await
        .map_err(|e| Refusal::no_address(format!("{target_host} — DNS: {e:#}")))?;
    first_allowed_resolved_address(&addrs, target_host, port).and_then(|addr| reject_own_address(addr, target_host))
}

/// Connection to an address. If it didn't open, the address may be stale in the cache — drop it.
pub(in crate::exit) async fn open_tcp(
    cfg: &ExitConfig,
    target_host: &str,
    addr: SocketAddr,
) -> std::result::Result<TcpStream, Refusal> {
    let Ok(Ok(tcp)) = timeout(EXIT_TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await else {
        cfg.resolver.forget(target_host);
        return Err(Refusal {
            code: EXIT_CODE_CONNECT_FAILED,
            line: format!("{target_host} ({addr}) — connection didn't open"),
        });
    };
    Ok(tcp)
}

#[cfg(test)]
#[path = "dial_tests.rs"]
mod tests;
