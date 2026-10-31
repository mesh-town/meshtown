//! Step-by-step check of one domain over the network: DNS (own DoH), TCP, TLS with the real server name
//! and HTTP GET counting the bytes received. It doesn't classify the steps into a result class —
//! that is `classify.rs`; here is only the network and building the final record.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use crate::site_checks::doh::{self, DnsAnswer, Resolver};
use crate::site_checks::http_min::{self, HttpGetRequest, HttpLimits, HttpResponse};

use super::classify::classify;
use super::config::{CheckConfig, ANTIBOT_MARKERS, CHECK_RECORD_VERSION, GEO_MARKERS};
use super::record::CheckRecord;
use super::steps::{BodyMark, Steps, TcpStep, TlsStep};

/// Checks one domain. An error is returned only if DoH itself failed — no connection,
/// wrong HTTP code, response not parsed, the resolver answered with an error code: then there is nothing to check and
/// nothing to publish. An "no addresses" answer is not an error: that is the "down" result.
pub async fn check_domain(
    config: &CheckConfig,
    resolver: &Resolver,
    domain: &str,
) -> Result<CheckRecord> {
    let mut steps = Steps::default();
    // The domain is marked --block-site: we don't touch the network at all, the class is preset.
    if config.blocked_domains.contains(domain) {
        return Ok(blocked_record(&mut steps));
    }
    let doh_answer = resolver
        .resolve(domain)
        .await
        .context("own resolver (DoH) didn't respond")?;
    let dns_ms = saturating_millis_u16(doh_answer.elapsed);
    steps.dns_ok = !doh_answer.addrs.is_empty();

    steps.dns_mismatch = provider_mismatch(config, resolver, domain, &doh_answer).await;

    // An empty DNS answer is "down", not "unusable address": without this branch rule 1
    // (`unusable_addr`) would fire before rule 2 (`dns_ok`) and return
    // "DNS spoofing" where there are simply no addresses.
    if !steps.dns_ok {
        return Ok(build_record(&steps, dns_ms, 0, 0, 0));
    }

    let addr = doh_answer
        .addrs
        .iter()
        .copied()
        .find(|a| !doh::is_unusable(*a));
    let Some(addr) = addr else {
        steps.unusable_addr = true;
        return Ok(build_record(&steps, dns_ms, 0, 0, 0));
    };

    let (tcp, tcp_ms, stream) = connect_tcp(addr, config.limits).await;
    steps.tcp = tcp;
    let Some(stream) = stream else {
        return Ok(build_record(&steps, dns_ms, tcp_ms, 0, 0));
    };

    // The read timeout adapts to latency: min(max(RTT×3, 1.5 s), 12 s).
    let limits = adaptive_limits(config.limits, tcp_ms);

    let tls_started = Instant::now();
    let tls = connect_tls(stream, domain, limits).await;
    let tls_ms = saturating_millis_u16(tls_started.elapsed());
    let stream = match tls {
        Ok(s) => {
            steps.tls = TlsStep::Ok;
            s
        }
        Err(step) => {
            steps.tls = step;
            return Ok(build_record(&steps, dns_ms, tcp_ms, tls_ms, 0));
        }
    };

    let timings = Timings {
        dns_ms,
        tcp_ms,
        tls_ms,
    };
    finish_http(config, domain, stream, &mut steps, timings, limits).await
}

/// Whether the provider's own DNS answers with addresses the DoH answer does not contain.
async fn provider_mismatch(
    config: &CheckConfig,
    resolver: &Resolver,
    domain: &str,
    doh_answer: &DnsAnswer,
) -> bool {
    let Some(provider) = config.provider_dns else {
        return false;
    };
    match resolver.resolve_udp(provider, domain).await {
        Ok(answer) => !answer.addrs.iter().any(|a| doh_answer.addrs.contains(a)),
        Err(_) => false,
    }
}

/// The read timeout adapts to latency: min(max(RTT×3, 1.5 s), 12 s).
fn adaptive_limits(limits: HttpLimits, tcp_ms: u16) -> HttpLimits {
    let rtt = Duration::from_millis(u64::from(tcp_ms));
    let read = (rtt * 3).clamp(Duration::from_millis(1500), Duration::from_secs(12));
    HttpLimits { read, ..limits }
}

/// A short exit for a domain marked `--block-site`: we don't touch the network at all.
fn blocked_record(steps: &mut Steps) -> CheckRecord {
    steps.dns_ok = true;
    steps.tcp = TcpStep::Refused;
    let mut record = build_record(steps, 0, 0, 0, 0);
    record.reason = "domain is marked --block-domain".to_string();
    record
}

/// Step times before HTTP; folded into one so `finish_http` doesn't grow in parameter count.
struct Timings {
    dns_ms: u16,
    tcp_ms: u16,
    tls_ms: u16,
}

/// The tail of a check after TLS is established: HTTP GET and building the final record.
async fn finish_http(
    cfg: &CheckConfig,
    domain: &str,
    mut stream: tokio_rustls::client::TlsStream<TcpStream>,
    steps: &mut Steps,
    timings: Timings,
    limits: HttpLimits,
) -> Result<CheckRecord> {
    let http_started = Instant::now();
    let req = HttpGetRequest {
        host: domain,
        path: "/",
        accept: "text/html,application/xhtml+xml,*/*;q=0.8",
        user_agent: &cfg.user_agent,
    };
    let http_ms;
    match http_min::http_get(&mut stream, req, limits).await {
        Ok(resp) => {
            http_ms = saturating_millis_u16(http_started.elapsed());
            steps.http_status = resp.status;
            steps.bytes_received = resp.raw_bytes as u32;
            steps.stalled = resp.stalled;
            steps.body_mark = body_mark(&resp, &cfg.geo_markers);
        }
        Err(_) => {
            http_ms = saturating_millis_u16(http_started.elapsed());
            steps.http_status = 0;
        }
    }
    Ok(build_record(
        steps,
        timings.dns_ms,
        timings.tcp_ms,
        timings.tls_ms,
        http_ms,
    ))
}

/// Opens TCP to an address from the DoH answer.
///
/// Here and only here the check goes to the network with its own socket. When TUN appears, marking the
/// socket to bypass the tunnel (`SO_MARK` on Linux, `IP_BOUND_IF` on macOS) will go into this function.
async fn connect_tcp(addr: Ipv4Addr, limits: HttpLimits) -> (TcpStep, u16, Option<TcpStream>) {
    let started = Instant::now();
    let target = SocketAddr::from((addr, 443));
    match timeout(limits.connect, TcpStream::connect(target)).await {
        Err(_) => (
            TcpStep::Timeout,
            saturating_millis_u16(started.elapsed()),
            None,
        ),
        Ok(Ok(s)) => (
            TcpStep::Ok,
            saturating_millis_u16(started.elapsed()),
            Some(s),
        ),
        Ok(Err(e)) => {
            let step = match e.kind() {
                std::io::ErrorKind::ConnectionRefused => TcpStep::Refused,
                std::io::ErrorKind::ConnectionReset => TcpStep::Reset,
                std::io::ErrorKind::TimedOut => TcpStep::Timeout,
                _ => TcpStep::Reset,
            };
            (step, saturating_millis_u16(started.elapsed()), None)
        }
    }
}

/// TLS handshake with the real server name — the "network or site" boundary.
async fn connect_tls(
    stream: TcpStream,
    domain: &str,
    limits: HttpLimits,
) -> std::result::Result<tokio_rustls::client::TlsStream<TcpStream>, TlsStep> {
    let cfg: Arc<ClientConfig> = match http_min::tls_client_config() {
        Ok(c) => Arc::new(c),
        Err(e) => return Err(TlsStep::Invalid(format!("{e:#}"))),
    };
    let name = match ServerName::try_from(domain.to_string()) {
        Ok(n) => n,
        Err(e) => return Err(TlsStep::Invalid(format!("name is not valid for TLS: {e}"))),
    };
    let connector = TlsConnector::from(cfg);
    match timeout(limits.connect, connector.connect(name, stream)).await {
        Err(_) => Err(TlsStep::Timeout),
        Ok(Ok(s)) => Ok(s),
        Ok(Err(e)) => Err(tls_step_of_error(&e)),
    }
}

/// Parses a handshake error: rustls puts its error inside `io::Error`.
fn tls_step_of_error(err: &std::io::Error) -> TlsStep {
    if let Some(inner) = err
        .get_ref()
        .and_then(|e| e.downcast_ref::<rustls::Error>())
    {
        return match inner {
            rustls::Error::AlertReceived(desc) => TlsStep::Alert(format!("{desc:?}")),
            other => TlsStep::Invalid(format!("{other}")),
        };
    }
    match err.kind() {
        std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset => TlsStep::Closed,
        std::io::ErrorKind::TimedOut => TlsStep::Timeout,
        other => TlsStep::Invalid(format!("{other:?}")),
    }
}

/// Looks in the response for signs of a robot check and a region stub.
fn body_mark(resp: &HttpResponse, extra: &[String]) -> BodyMark {
    if resp.header("cf-mitigated").is_some() {
        return BodyMark::Antibot;
    }
    let scan_len = resp.body.len().min(8 * 1024);
    let text = String::from_utf8_lossy(&resp.body[..scan_len]).to_lowercase();
    if ANTIBOT_MARKERS.iter().any(|m| text.contains(m)) {
        return BodyMark::Antibot;
    }
    if GEO_MARKERS.iter().any(|m| text.contains(m))
        || extra.iter().any(|m| text.contains(m.as_str()))
    {
        return BodyMark::Geo;
    }
    BodyMark::Clean
}

/// Builds a record from steps and times.
fn build_record(steps: &Steps, dns_ms: u16, tcp_ms: u16, tls_ms: u16, http_ms: u16) -> CheckRecord {
    let (verdict, reason) = classify(steps);
    CheckRecord {
        v: CHECK_RECORD_VERSION,
        verdict,
        dns_ms,
        tcp_ms,
        tls_ms,
        http_ms,
        status: steps.http_status,
        bytes: steps.bytes_received,
        stalled: steps.stalled,
        at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        reason,
    }
}

/// Milliseconds with a ceiling: the record field is two bytes wide.
fn saturating_millis_u16(d: Duration) -> u16 {
    d.as_millis().min(u128::from(u16::MAX)) as u16
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
