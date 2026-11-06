//! Step-by-step check of one domain over the network: DNS (own DoH), TCP, TLS with the real server name
//! and HTTP GET counting the bytes received. It doesn't classify the steps into a result class —
//! that is `classify.rs`; here is only the network and building the final record.

use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::site_checks::doh::{self, DnsAnswer, Resolver};
use crate::site_checks::http_min::{HttpLimits, HttpResponse};

use super::classify::classify;
use super::config::{CheckConfig, ANTIBOT_MARKERS, CHECK_RECORD_VERSION, GEO_MARKERS};
use super::probe_stream::finish_stream;
use super::record::CheckRecord;
use super::steps::{BodyMark, Steps, TcpStep};

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

    finish_stream(config, domain, stream, &mut steps, (dns_ms, tcp_ms)).await
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

/// A short exit for a domain marked `--block-site`: we don't touch the network at all.
pub(super) fn blocked_record(steps: &mut Steps) -> CheckRecord {
    steps.dns_ok = true;
    steps.tcp = TcpStep::Refused;
    let mut record = build_record(steps, 0, 0, 0, 0);
    record.reason = "domain is marked --block-site".to_string();
    record
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

/// Looks in the response for signs of a robot check and a region stub.
pub(super) fn body_mark(resp: &HttpResponse, extra: &[String]) -> BodyMark {
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
pub(super) fn build_record(
    steps: &Steps,
    dns_ms: u16,
    tcp_ms: u16,
    tls_ms: u16,
    http_ms: u16,
) -> CheckRecord {
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
pub(super) fn saturating_millis_u16(d: Duration) -> u16 {
    d.as_millis().min(u128::from(u16::MAX)) as u16
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
