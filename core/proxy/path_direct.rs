//! The direct route: the target address and a TCP connection to it.
//! A child of `path`: it reuses the refusal type and the upstream wrapper.

use std::net::SocketAddr;

use anyhow::{anyhow, Context, Result};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::node::output::say;
use crate::node::tcp_tune::disable_nagle;
use crate::site_checks::doh;

use super::super::outcome::ProxyOutcome;
use super::super::serve::{ProxyConfig, DIRECT_TCP_CONNECT_TIMEOUT};
use super::super::socks5::Target;
use super::{OpenRefused, OpenedUpstream, Upstream};

/// The target address for the direct route: an IP address as is, a domain via the test table, then the own resolver
/// over HTTPS (with a cache: a repeated name goes without contacting DoH).
pub(in crate::proxy) async fn address_of(
    cfg: &ProxyConfig,
    target: &Target,
    port: u16,
) -> Result<SocketAddr> {
    let domain = match target {
        Target::Ip(ip) => return Ok(SocketAddr::new(*ip, port)),
        Target::Domain(domain) => domain,
    };
    if let Some(addr) = cfg.host_overrides.get(domain) {
        return Ok(*addr);
    }
    let addrs = cfg
        .resolver
        .resolve_cached(domain)
        .await
        .with_context(|| format!("{domain}: the own resolver did not answer"))?;
    let ip = addrs
        .iter()
        .copied()
        .find(|a| !doh::is_unusable(*a))
        .ok_or_else(|| anyhow!("{domain}: no usable address"))?;
    Ok(SocketAddr::from((ip, port)))
}

/// If it did not open, the address may have gone stale in the resolver cache: reset it (does nothing for an IP target).
fn forget_address(cfg: &ProxyConfig, target: &Target) {
    if let Target::Domain(domain) = target {
        cfg.resolver.forget(domain);
    }
}

/// The direct route: the target address and a connection with the `DIRECT_TCP_CONNECT_TIMEOUT` timeout.
pub(super) async fn open_direct_tcp(
    cfg: &ProxyConfig,
    target: &Target,
    port: u16,
    tag: &'static str,
) -> Result<OpenedUpstream, OpenRefused> {
    let addr = match address_of(cfg, target, port).await {
        Ok(addr) => addr,
        Err(e) => {
            say(tag, format!("{e:#}"));
            return Err(OpenRefused::new(ProxyOutcome::refused(
                "direct",
                "address not found",
            )));
        }
    };
    connect_direct(cfg, target, addr, tag).await
}

/// Connects to `addr` within the timeout; a failed attempt drops the cached address of a domain target.
async fn connect_direct(
    cfg: &ProxyConfig,
    target: &Target,
    addr: SocketAddr,
    tag: &'static str,
) -> Result<OpenedUpstream, OpenRefused> {
    match timeout(DIRECT_TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(tcp)) => {
            disable_nagle(&tcp);
            Ok(OpenedUpstream {
                route_label: "direct".to_string(),
                upstream: Upstream::Tcp(tcp),
            })
        }
        Ok(Err(_)) => {
            forget_address(cfg, target);
            say(tag, format!("{addr}: connection did not open"));
            Err(OpenRefused::new(ProxyOutcome::refused(
                "direct",
                "connection did not open",
            )))
        }
        Err(_) => {
            forget_address(cfg, target);
            say(tag, format!("{addr}: connection did not open"));
            Err(OpenRefused {
                outcome: ProxyOutcome::refused("direct", "connection did not open"),
                timed_out: true,
                gateway_refused: false,
            })
        }
    }
}
