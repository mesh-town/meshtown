//! Queries three geoip sources in turn with its own TCP+TLS over `http_min`.
//!
//! What it does: resolves the source name with the system resolver or takes an IP literal without
//! DNS, brings up TCP and TLS, sends GET via `http_min::http_get`, parses the answer via
//! `sources::read_answer`; when one source fails within its budget — the next one by
//! `sources::SOURCE_ORDER`.
//! What it doesn't do: doesn't decide the schedule of checks and doesn't write to the network document — those are `geoip::task`
//! and `geoip::board`.
//! Links: the only place in the `geoip` module that opens a socket outward.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use crate::doh;
use crate::http_min::{self, HttpLimits, HttpGetRequest};

use super::country::Country;
use super::sources::{self, read_answer, Source};

/// The User-Agent of the request — a common browser one, with no identifying marks of the node.
const USER_AGENT: &str = "Mozilla/5.0";
/// The whole budget of each source: DNS, TCP, TLS, the request.
const SOURCE_BUDGET: Duration = Duration::from_secs(5);
/// The limits of one request inside a source's budget.
const SOURCE_REQUEST_LIMITS: HttpLimits = HttpLimits {
    connect: Duration::from_secs(3),
    read: Duration::from_secs(3),
    total: Duration::from_secs(4),
    max_bytes: 16 * 1024,
};

/// Asks the node's country from the sources by `sources::SOURCE_ORDER`; the first success is the answer, if all refused —
/// the reasons joined with `; `.
pub async fn lookup_country() -> Result<(Country, Source), String> {
    let mut reasons = Vec::new();
    for source in sources::SOURCE_ORDER {
        match timeout(SOURCE_BUDGET, lookup_from_source(source)).await {
            Ok(Ok(country)) => return Ok((country, source)),
            Ok(Err(reason)) => reasons.push(reason),
            Err(_) => reasons.push(format!("{}: timeout", source.spec().host)),
        }
    }
    Err(reasons.join("; "))
}

/// One source as a whole: the request and parsing the answer; the refusal reason carries the source name.
async fn lookup_from_source(source: Source) -> Result<Country, String> {
    let spec = source.spec();
    let resp = https_get(spec.host, spec.path, spec.accept, SOURCE_REQUEST_LIMITS)
        .await
        .map_err(|e| format!("{}: {e:#}", spec.host))?;
    read_answer(source, resp.status, &resp.body).map_err(|e| format!("{}: {e}", spec.host))
}

/// GET over HTTPS to the address or name `host`: for a name, resolving with the system resolver, TCP and
/// TLS under `limits.connect`, `http_min::http_get` over the ready stream.
async fn https_get(
    host: &str,
    path: &str,
    accept: &str,
    limits: HttpLimits,
) -> Result<http_min::HttpResponse> {
    let addr = timeout(limits.connect, resolve_v4(host))
        .await
        .context("address resolution timed out")??;
    // Here and only here the country lookup opens its own socket outward; the "bypass the tunnel" mark
    // (as in `reachability::probe::connect_tcp`) will go here when TUN appears.
    let tcp = timeout(
        limits.connect,
        TcpStream::connect(SocketAddr::from((addr, 443))),
    )
    .await
    .context("TCP connection timed out")??;
    let tls_config = Arc::new(http_min::tls_client_config()?);
    let name = ServerName::try_from(host.to_string())
        .map_err(|e| anyhow::anyhow!("name is not valid for TLS: {e}"))?;
    let connector = TlsConnector::from(tls_config);
    let mut stream = timeout(limits.connect, connector.connect(name, tcp))
        .await
        .context("TLS handshake timed out")??;
    let req = HttpGetRequest {
        host,
        path,
        accept,
        user_agent: USER_AGENT,
    };
    http_min::http_get(&mut stream, req, limits).await
}

/// An IPv4 address for `host`: an IP literal without touching the network, a name — the system resolver;
/// fake/loopback addresses are discarded as unusable.
async fn resolve_v4(host: &str) -> Result<Ipv4Addr> {
    let addrs = tokio::net::lookup_host((host, 443))
        .await
        .context("name did not resolve")?;
    addrs
        .filter_map(|a| match a {
            SocketAddr::V4(v4) if !doh::is_unusable(*v4.ip()) => Some(*v4.ip()),
            _ => None,
        })
        .next()
        .ok_or_else(|| anyhow::anyhow!("no usable IPv4 address"))
}
