//! The common connection path of the proxy: route choice, connecting to the target directly or through an exit, and
//! relaying bytes. It writes nothing to the client — the protocol's "face" (`serve`, `http`) writes the reply.

use crate::transport::net::{BoxRecv, BoxSend, PeerId};
use anyhow::Result;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::exit;
use crate::node::output::{say, short_peer_id};
use crate::proxy::route::{self, RouteCtx};
use crate::proxy::socks5::Target;

use super::outcome::ProxyOutcome;
use super::serve::ProxyConfig;

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
                "the exit list could not be read",
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

#[path = "path_direct.rs"]
mod direct;
#[path = "path_exits.rs"]
mod exits;
#[path = "path_udp.rs"]
mod udp;
pub(in crate::proxy) use direct::address_of;
use direct::open_direct_tcp;
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
