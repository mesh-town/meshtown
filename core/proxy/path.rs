//! The common connection path of the proxy: route choice, connecting to the target directly or through an exit, and
//! relaying bytes. It writes nothing to the client — the protocol's "face" (`serve`, `http`) writes the reply.

use std::net::SocketAddr;

use crate::transport::net::{BoxRecv, BoxSend, PeerId};
use anyhow::{anyhow, Context, Result};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::exit;
use crate::node::output::{say, short_peer_id};
use crate::node::tcp_tune::disable_nagle;
use crate::proxy::route::{self, RouteCtx};
use crate::proxy::socks5::Target;
use crate::site_checks::doh;

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
    Exit {
        peer: PeerId,
        send: BoxSend,
        recv: BoxRecv,
    },
    /// An in-process stream (a mesh peer's port or an answer page).
    Stream {
        send: BoxSend,
        recv: BoxRecv,
    },
}

impl OpenedUpstream {
    /// An upstream that is an already open stream.
    pub(super) fn stream(route_label: String, send: BoxSend, recv: BoxRecv) -> OpenedUpstream {
        OpenedUpstream {
            route_label,
            upstream: Upstream::Stream { send, recv },
        }
    }
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
        Self {
            outcome,
            timed_out: false,
            gateway_refused: false,
        }
    }

    fn no_gateway(outcome: ProxyOutcome) -> Self {
        Self {
            outcome,
            timed_out: false,
            gateway_refused: true,
        }
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
            return Err(OpenRefused::new(ProxyOutcome::refused(
                "no route",
                "the list of exits could not be read",
            )));
        }
    };
    let target_text = target.to_string();
    let candidates = match route.candidates() {
        Ok(candidates) => candidates,
        Err(reason) => {
            say("route", format!("{target_text}:{port} → refused: {reason}"));
            return Err(OpenRefused::new(ProxyOutcome::refused("no route", reason)));
        }
    };
    let Some(candidates) = candidates else {
        say("route", format!("{target_text}:{port} → direct"));
        return open_direct_tcp(cfg, target, port, tag).await;
    };
    let first = route::first_exit(&candidates);
    say(
        "route",
        format!("{target_text}:{port} → exit {}", short_peer_id(&first)),
    );
    let link = open_through_exits(cfg, &candidates, &target_text, port).await?;
    Ok(OpenedUpstream {
        route_label: link.route_label,
        upstream: Upstream::Exit {
            peer: link.peer,
            send: link.send,
            recv: link.recv,
        },
    })
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
            let (bytes_from_client, bytes_to_client) =
                tokio::io::copy_bidirectional(&mut client, &mut target_tcp)
                    .await
                    .unwrap_or((0, 0));
            ProxyOutcome::success(
                route_label,
                bytes_to_client,
                bytes_from_client + prefix.len() as u64,
            )
        }
        Upstream::Stream { mut send, recv } => {
            if !prefix.is_empty() && send.write_all(prefix).await.is_err() {
                return ProxyOutcome::refused(route_label, "request not forwarded");
            }
            let (bytes_to_client, bytes_from_client) = exit::splice(send, recv, client).await;
            ProxyOutcome::success(
                route_label,
                bytes_to_client,
                bytes_from_client + prefix.len() as u64,
            )
        }
        Upstream::Exit { peer, send, recv } => {
            relay_exit(route_label, route, peer, (send, recv), client, prefix).await
        }
    }
}

/// The exit arm of `relay_bytes`: splices while the exit stays chosen.
async fn relay_exit(
    route_label: String,
    route: &RouteCtx,
    peer: PeerId,
    (mut send, recv): (BoxSend, BoxRecv),
    client: TcpStream,
    prefix: &[u8],
) -> ProxyOutcome {
    if !prefix.is_empty() && send.write_all(prefix).await.is_err() {
        return ProxyOutcome::refused(route_label, "request not forwarded");
    }
    match splice_while_chosen(route, peer, send, recv, client).await {
        Some((bytes_to_client, bytes_from_client)) => ProxyOutcome::success(
            route_label,
            bytes_to_client,
            bytes_from_client + prefix.len() as u64,
        ),
        None => {
            say(
                "route",
                format!(
                    "exit {} is no longer selected — connection closed",
                    short_peer_id(&peer)
                ),
            );
            ProxyOutcome::interrupted(route_label)
        }
    }
}

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
            return Err(OpenRefused::new(ProxyOutcome::refused(
                "direct",
                "address not found",
            )));
        }
    };
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

/// Logs an exit's refusal code and turns it into the outcome shown to the user.
fn exit_refusal(peer: PeerId, code: u8, route_label: String) -> ProxyOutcome {
    say(
        "exit",
        format!(
            "{}: refused, code {code} — {}",
            short_peer_id(&peer),
            exit::exit_code_text(code)
        ),
    );
    let reason = format!("code {code} — {}", exit::exit_code_text(code));
    ProxyOutcome::refused(route_label, reason)
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

#[path = "path_exits.rs"]
mod exits;
#[path = "path_udp.rs"]
mod udp;
#[cfg(test)]
use exits::no_answer_reason;
use exits::open_through_exits;
pub(super) use udp::{open_udp_route, ExitLink, UdpRefusal, UdpRoute};

#[cfg(test)]
#[path = "path_busy_tests.rs"]
pub(in crate::proxy) mod busy_tests;
#[cfg(test)]
#[path = "path_gateway_tests.rs"]
mod gateway_tests;
#[cfg(test)]
#[path = "path_tests.rs"]
mod tests;
