//! The tail of a check once a byte stream to the site exists: TLS with the real server name, HTTP GET
//! counting the bytes received, and the final record. The stream is generic: a direct TCP socket
//! (`probe.rs`) or a stream opened through a member's exit (`probe_exit.rs`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use crate::site_checks::http_min::{self, HttpGetRequest, HttpLimits};

use super::config::CheckConfig;
use super::probe::{body_mark, build_record, saturating_millis_u16};
use super::record::CheckRecord;
use super::steps::{Steps, TlsStep};

/// TLS and HTTP over an established stream; `(dns_ms, tcp_ms)` are the times of the steps before it.
pub(super) async fn finish_stream<S>(
    config: &CheckConfig,
    domain: &str,
    stream: S,
    steps: &mut Steps,
    (dns_ms, tcp_ms): (u16, u16),
) -> Result<CheckRecord>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // The read timeout adapts to latency: min(max(RTT×3, 1.5 s), 12 s).
    let limits = adaptive_limits(config.limits, tcp_ms);

    let tls_started = Instant::now();
    let tls = connect_tls(stream, domain, limits, config).await;
    let tls_ms = saturating_millis_u16(tls_started.elapsed());
    let stream = match tls {
        Ok(s) => {
            steps.tls = TlsStep::Ok;
            s
        }
        Err(step) => {
            steps.tls = step;
            return Ok(build_record(steps, dns_ms, tcp_ms, tls_ms, 0));
        }
    };
    let timings = Timings {
        dns_ms,
        tcp_ms,
        tls_ms,
    };
    finish_http(config, domain, stream, steps, timings, limits).await
}

/// The read timeout adapts to latency: min(max(RTT×3, 1.5 s), 12 s).
fn adaptive_limits(limits: HttpLimits, tcp_ms: u16) -> HttpLimits {
    let rtt = Duration::from_millis(u64::from(tcp_ms));
    let read = (rtt * 3).clamp(Duration::from_millis(1500), Duration::from_secs(12));
    HttpLimits { read, ..limits }
}

/// Step times before HTTP; folded into one so `finish_http` doesn't grow in parameter count.
struct Timings {
    dns_ms: u16,
    tcp_ms: u16,
    tls_ms: u16,
}

/// The tail of a check after TLS is established: HTTP GET and building the final record.
async fn finish_http<S>(
    cfg: &CheckConfig,
    domain: &str,
    mut stream: S,
    steps: &mut Steps,
    timings: Timings,
    limits: HttpLimits,
) -> Result<CheckRecord>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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

/// TLS handshake with the real server name — the "network or site" boundary.
async fn connect_tls<S>(
    stream: S,
    domain: &str,
    limits: HttpLimits,
    config: &CheckConfig,
) -> std::result::Result<tokio_rustls::client::TlsStream<S>, TlsStep>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let cfg: Arc<ClientConfig> = match http_min::tls_client_config_with_roots(&config.extra_roots) {
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
