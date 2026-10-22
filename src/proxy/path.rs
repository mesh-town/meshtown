//! The common connection path of the proxy: route choice, connecting to the target directly or through an exit, and
//! relaying bytes. It writes nothing to the client — the protocol's "face" (`serve`, `http`) writes the reply.

use std::net::SocketAddr;

use crate::net::{BoxRecv, BoxSend, PeerId};
use anyhow::{anyhow, Context, Result};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::doh;
use crate::exit;
use crate::output::{say, short_peer_id};
use crate::route::{self, Route, RouteCtx};
use crate::socks5::Target;
use crate::tcp_tune::disable_nagle;

use super::outcome::ProxyOutcome;
use super::serve::{ProxyConfig, DIRECT_TCP_CONNECT_TIMEOUT};

/// An open connection to the target and the route label for the summary line.
pub(super) struct OpenedUpstream {
    /// The route in words: `direct` or `exit <short>`.
    pub route_label: String,
    upstream: Upstream,
}

/// Where the open connection leads.
enum Upstream {
    Tcp(TcpStream),
    Exit { peer: PeerId, send: BoxSend, recv: BoxRecv },
}

/// A refusal: the ready outcome and a flag "the direct connection did not open within `DIRECT_TCP_CONNECT_TIMEOUT`".
pub(super) struct OpenRefused {
    pub outcome: ProxyOutcome,
    pub timed_out: bool,
    /// The exit has no UDP gateway (refused the gateway target with code 7, or is remembered as such).
    pub gateway_refused: bool,
}

impl OpenRefused {
    fn new(outcome: ProxyOutcome) -> Self {
        Self { outcome, timed_out: false, gateway_refused: false }
    }

    fn no_gateway(outcome: ProxyOutcome) -> Self {
        Self { outcome, timed_out: false, gateway_refused: true }
    }
}

/// Chooses a route and opens a connection to the target. It writes its own errors to the log under the `tag` label.
pub(super) async fn open_chosen_route(
    cfg: &ProxyConfig,
    target: &Target,
    port: u16,
    tag: &'static str,
) -> Result<OpenedUpstream, OpenRefused> {
    let route = match cfg.route.choose_now().await {
        Ok(route) => route,
        Err(e) => {
            say(tag, format!("{e:#}"));
            return Err(OpenRefused::new(ProxyOutcome::refused("no route", "the list of exits could not be read")));
        }
    };
    let target_text = target.to_string();
    match route {
        Route::Direct => {
            say("route", format!("{target_text}:{port} → direct"));
            open_direct_tcp(cfg, target, port, tag).await
        }
        Route::Exits(candidates) => {
            let first = route::first_exit(&candidates);
            say("route", format!("{target_text}:{port} → exit {}", short_peer_id(&first)));
            open_through_exits(cfg, &candidates, &target_text, port).await
        }
        Route::Refuse(reason) => {
            say("route", format!("{target_text}:{port} → refused: {reason}"));
            Err(OpenRefused::new(ProxyOutcome::refused("no route", reason)))
        }
    }
}

/// Writes `prefix` upstream and relays bytes in both directions until the transfer ends (or until the
/// exit is removed from `route.exits`).
pub(super) async fn relay_bytes(
    opened: OpenedUpstream,
    route: &RouteCtx,
    client: TcpStream,
    prefix: &[u8],
) -> ProxyOutcome {
    let route_label = opened.route_label;
    match opened.upstream {
        Upstream::Tcp(mut target_tcp) => {
            if !prefix.is_empty() && target_tcp.write_all(prefix).await.is_err() {
                return ProxyOutcome::refused(route_label, "request not forwarded");
            }
            let mut client = client;
            let (bytes_from_client, bytes_to_client) = tokio::io::copy_bidirectional(&mut client, &mut target_tcp)
                .await
                .unwrap_or((0, 0));
            ProxyOutcome::success(route_label, bytes_to_client, bytes_from_client + prefix.len() as u64)
        }
        Upstream::Exit { peer, mut send, recv } => {
            if !prefix.is_empty() && send.write_all(prefix).await.is_err() {
                return ProxyOutcome::refused(route_label, "request not forwarded");
            }
            match splice_while_chosen(route, peer, send, recv, client).await {
                Some((bytes_to_client, bytes_from_client)) => ProxyOutcome::success(route_label, bytes_to_client, bytes_from_client + prefix.len() as u64),
                None => {
                    say("route", format!("exit {} is no longer selected — connection closed", short_peer_id(&peer)));
                    ProxyOutcome::interrupted(route_label)
                }
            }
        }
    }
}

/// The target address for the direct route: an IP address as is, a domain via the test table, then the own resolver
/// over HTTPS (with a cache: a repeated name goes without contacting DoH).
pub(in crate::proxy) async fn address_of(cfg: &ProxyConfig, target: &Target, port: u16) -> Result<SocketAddr> {
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
async fn open_direct_tcp(
    cfg: &ProxyConfig,
    target: &Target,
    port: u16,
    tag: &'static str,
) -> Result<OpenedUpstream, OpenRefused> {
    let addr = match address_of(cfg, target, port).await {
        Ok(addr) => addr,
        Err(e) => {
            say(tag, format!("{e:#}"));
            return Err(OpenRefused::new(ProxyOutcome::refused("direct", "address not found")));
        }
    };
    match timeout(DIRECT_TCP_CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
        Ok(Ok(tcp)) => {
            disable_nagle(&tcp);
            Ok(OpenedUpstream { route_label: "direct".to_string(), upstream: Upstream::Tcp(tcp) })
        }
        Ok(Err(_)) => {
            forget_address(cfg, target);
            say(tag, format!("{addr}: connection did not open"));
            Err(OpenRefused::new(ProxyOutcome::refused("direct", "connection did not open")))
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

/// The route through one of the chosen exits: tries the candidates in order, moving to the
/// next one on a transport error and, for the UDP gateway target, on a refusal of the gateway; any other
/// exit reply code is final. When every candidate lacks the gateway, the last such refusal is returned.
async fn open_through_exits(
    cfg: &ProxyConfig,
    candidates: &[PeerId],
    target_text: &str,
    port: u16,
) -> Result<OpenedUpstream, OpenRefused> {
    let mut any_exit_stream_busy = false;
    let mut no_gateway = None;
    let gateway = exit::is_gateway_target(target_text, port);
    for &peer in candidates {
        let route_label = format!("exit {}", short_peer_id(&peer));
        if gateway && cfg.pool.gateway_refused_recently(peer) {
            no_gateway = Some(remembered_gateway_refusal(peer, route_label));
            continue;
        }
        let (send, recv, code) = match exit::ask_exit_to_connect(&cfg.stack, &cfg.pool, peer, target_text, port).await {
            Ok(answer) => answer,
            Err(e) => {
                any_exit_stream_busy |= e.downcast_ref::<exit::StreamBusy>().is_some();
                say("route", format!("exit {} did not answer: {e:#}", short_peer_id(&peer)));
                continue;
            }
        };
        if code != exit::EXIT_CODE_OK {
            let gateway_refusal = gateway && code == exit::EXIT_CODE_ADDRESS_FORBIDDEN;
            if gateway_refusal {
                cfg.pool.note_gateway_refused(peer);
            }
            say(
                "exit",
                format!("{}: refused, code {code} — {}", short_peer_id(&peer), exit::exit_code_text(code)),
            );
            let reason = format!("code {code} — {}", exit::exit_code_text(code));
            let outcome = ProxyOutcome::refused(route_label, reason);
            if gateway_refusal {
                no_gateway = Some(OpenRefused::no_gateway(outcome));
                continue;
            }
            return Err(OpenRefused::new(outcome));
        }
        return Ok(OpenedUpstream { route_label, upstream: Upstream::Exit { peer, send, recv } });
    }
    Err(no_gateway.unwrap_or_else(|| {
        OpenRefused::new(ProxyOutcome::refused("no route", no_answer_reason(any_exit_stream_busy)))
    }))
}

/// The refusal for the UDP gateway target on an exit that already refused it: the exit is not asked again.
fn remembered_gateway_refusal(peer: PeerId, route_label: String) -> OpenRefused {
    let code = exit::EXIT_CODE_ADDRESS_FORBIDDEN;
    say("exit", format!("{}: no UDP gateway (refused earlier)", short_peer_id(&peer)));
    let reason = format!("code {code} — {}", exit::exit_code_text(code));
    OpenRefused::no_gateway(ProxyOutcome::refused(route_label, reason))
}

/// The refusal reason when no exit answered: `any_exit_stream_busy` — at least one `open_bi` timed out
/// (the exit is online but its streams are busy), otherwise the exits did not answer at all.
fn no_answer_reason(any_exit_stream_busy: bool) -> &'static str {
    if any_exit_stream_busy {
        return "exit is busy (stream limit)";
    }
    "the selected exits did not answer"
}

/// Relays bytes while `peer` remains the chosen exit (the reset task's decision):
/// a race between `exit::splice` and `route.wait_until_exit_dropped(peer)`. `None` — the exit was removed from
/// `route.exits`, the `exit::splice` future is dropped (the guard in `splice.rs` aborts both subtasks),
/// and the client's TCP socket is closed along with it.
async fn splice_while_chosen(
    route: &RouteCtx,
    peer: PeerId,
    send: BoxSend,
    recv: BoxRecv,
    stream: TcpStream,
) -> Option<(u64, u64)> {
    tokio::select! {
        result = exit::splice(send, recv, stream) => Some(result),
        () = route.wait_until_exit_dropped(peer) => None,
    }
}

#[path = "path_udp.rs"]
mod udp;
#[allow(unused_imports)] // used by the association server
pub(super) use udp::{open_udp_route, ExitLink, UdpRefusal, UdpRoute};

#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "path_busy_tests.rs"]
mod busy_tests;
#[cfg(test)]
#[path = "path_gateway_tests.rs"]
mod gateway_tests;
