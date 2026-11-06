//! The check of one domain through a member's exit: the exit resolves the name and opens TCP, this node
//! does TLS and HTTP over the stream it got (`probe_stream.rs`) and classifies the steps as usual.
//! The request is the ordinary exit request (`crate::exit::ask_exit_to_connect`) — no protocol of its own.

use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use tokio::time::timeout;

use crate::exit::{
    ask_exit_to_connect, PeerConnPool, EXIT_CODE_CONNECT_FAILED, EXIT_CODE_NO_ADDRESS, EXIT_CODE_OK,
};
use crate::transport::net::{NetStack, PeerId};

use super::config::CheckConfig;
use super::probe::{blocked_record, build_record, saturating_millis_u16};
use super::probe_stream::finish_stream;
use super::record::CheckRecord;
use super::steps::{Steps, TcpStep};

/// Slack over `limits.connect` for the exit's own DNS lookup and the stream open.
const EXIT_SLACK: Duration = Duration::from_secs(4);

/// Checks `domain` (port 443) through the exit of `peer`. An error means there is nothing to record: the
/// exit is unreachable, busy, refused, or answered with a code that says nothing about the site.
pub async fn check_domain_via_exit(
    config: &CheckConfig,
    stack: &NetStack,
    pool: &PeerConnPool,
    peer: PeerId,
    domain: &str,
) -> Result<CheckRecord> {
    if config.blocked_domains.contains(domain) {
        return Ok(blocked_record(&mut Steps::default()));
    }
    let started = Instant::now();
    let ask = ask_exit_to_connect(stack, pool, peer, domain, 443);
    let (send, recv, code) = timeout(config.limits.connect + EXIT_SLACK, ask)
        .await
        .map_err(|_| anyhow!("exit did not answer in time"))?
        .context("exit request failed")?;
    let tcp_ms = saturating_millis_u16(started.elapsed());
    if code != EXIT_CODE_OK {
        return record_for_code(code, tcp_ms);
    }
    let mut steps = Steps {
        dns_ok: true,
        tcp: TcpStep::Ok,
        ..Steps::default()
    };
    let stream = tokio::io::join(recv, send);
    finish_stream(config, domain, stream, &mut steps, (0, tcp_ms)).await
}

/// The record for a non-OK reply code: the exit's DNS found nothing ("down"), its TCP connect failed
/// (as a TCP timeout); any other code is an error — it is about the exit, not about the site.
pub(super) fn record_for_code(code: u8, elapsed_ms: u16) -> Result<CheckRecord> {
    let mut steps = Steps::default();
    match code {
        EXIT_CODE_NO_ADDRESS => Ok(build_record(&steps, elapsed_ms, 0, 0, 0)),
        EXIT_CODE_CONNECT_FAILED => {
            steps.dns_ok = true;
            steps.tcp = TcpStep::Timeout;
            Ok(build_record(&steps, 0, elapsed_ms, 0, 0))
        }
        other => bail!("exit refused: {}", crate::exit::exit_code_text(other)),
    }
}

#[cfg(test)]
#[path = "probe_exit_tests.rs"]
mod tests;
