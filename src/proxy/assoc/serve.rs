//! The `UDP ASSOCIATE` command: a slot, the local datagram socket, the route, the reply and the journal.

use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::output::say;
use crate::socks5;

use super::super::outcome::{outcome_line, ProxyOutcome};
use super::super::path::{open_udp_route, ExitLink, UdpRefusal, UdpRoute};
use super::super::serve::{client_gone, ProxyConfig};
use super::relay::{relay, RelayLimits};
use super::stats::RelayStats;
use super::upstream::{ExitRx, ExitTx};

/// Associations at once on one node.
const MAX_UDP_ASSOCIATIONS: usize = 128;
/// An association without a datagram in either direction this long ends.
const ASSOC_IDLE: Duration = Duration::from_secs(600);
const JOURNAL_TARGET: &str = "udp associate";

/// The slots of a node's associations, for `ProxyConfig`.
pub fn udp_slots() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(MAX_UDP_ASSOCIATIONS))
}

pub(super) fn take_slot(slots: &Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
    slots.clone().try_acquire_owned().ok()
}

/// Serves one `UDP ASSOCIATE` request until the association ends.
pub(in crate::proxy) async fn serve_associate(mut stream: TcpStream, cfg: ProxyConfig, started: Instant) -> Result<()> {
    let Some(_slot) = take_slot(&cfg.udp_slots) else {
        say("socks", format!("{JOURNAL_TARGET}: refused, too many associations"));
        return reply_and_stop(&mut stream, socks5::REP_GENERAL).await;
    };
    let (client_ip, local_ip) = (stream.peer_addr()?.ip(), stream.local_addr()?.ip());
    let IpAddr::V4(local_v4) = local_ip else {
        return reply_and_stop(&mut stream, socks5::REP_GENERAL).await;
    };
    let Some(udp) = bind_local(local_v4).await else {
        return reply_and_stop(&mut stream, socks5::REP_GENERAL).await;
    };
    let bound = SocketAddrV4::new(local_v4, udp.local_addr()?.port());
    let opening = tokio::select! {
        opened = open_udp_route(&cfg) => Some(opened),
        () = client_gone(&stream) => None,
    };
    let route = match opening {
        None => return finish(started, &ProxyOutcome::abandoned(), None),
        Some(Err(refusal)) => return refuse(stream, refusal, started).await,
        Some(Ok(route)) => route,
    };
    match route {
        UdpRoute::Exit(link) => serve_exit(stream, udp, (client_ip, bound), link, &cfg, started).await,
        UdpRoute::Direct => {
            let outcome = ProxyOutcome::refused("direct", "no UDP route without an exit");
            socks5::write_reply(&mut stream, socks5::REP_CMD_NOT_SUPPORTED).await?;
            finish(started, &outcome, None)
        }
    }
}

async fn bind_local(ip: Ipv4Addr) -> Option<UdpSocket> {
    match UdpSocket::bind((ip, 0)).await {
        Ok(udp) => Some(udp),
        Err(e) => {
            say("socks", format!("{JOURNAL_TARGET}: cannot open a UDP socket: {e}"));
            None
        }
    }
}

async fn reply_and_stop(stream: &mut TcpStream, code: u8) -> Result<()> {
    socks5::write_reply(stream, code).await?;
    Ok(())
}

/// The refusal reply: 7 when the exits have no UDP gateway, 1 otherwise.
async fn refuse(mut stream: TcpStream, refusal: UdpRefusal, started: Instant) -> Result<()> {
    let code = if refusal.no_udp { socks5::REP_CMD_NOT_SUPPORTED } else { socks5::REP_GENERAL };
    socks5::write_reply(&mut stream, code).await?;
    finish(started, &refusal.outcome, None)
}

/// The association through the UDP gateway of an exit.
async fn serve_exit(
    mut stream: TcpStream,
    udp: UdpSocket,
    (client_ip, bound): (IpAddr, SocketAddrV4),
    link: ExitLink,
    cfg: &ProxyConfig,
    started: Instant,
) -> Result<()> {
    socks5::write_reply_bound(&mut stream, socks5::REP_OK, bound).await?;
    let (tx, rx) = (ExitTx(link.send), ExitRx(link.recv));
    let stop = cfg.route.wait_until_exit_dropped(link.peer);
    let stats = relay(stream, udp, client_ip, tx, rx, stop, RelayLimits { idle: ASSOC_IDLE }).await;
    let outcome = ProxyOutcome::success(link.route_label, stats.down_bytes, stats.up_bytes);
    finish(started, &outcome, Some(&stats))
}

/// The journal: the drop line (if any datagram was dropped) and the summary.
fn finish(started: Instant, outcome: &ProxyOutcome, stats: Option<&RelayStats>) -> Result<()> {
    stats.and_then(RelayStats::drops_line).into_iter().for_each(|line| say("socks", line));
    say("socks", outcome_line(JOURNAL_TARGET, outcome, started.elapsed().as_millis()));
    Ok(())
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
