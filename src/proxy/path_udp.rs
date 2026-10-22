//! The route of a UDP association: direct, or a stream to the UDP gateway of an exit.
//! A child of `path`: it reuses `open_through_exits` and its refusal type.

use crate::exit;
use crate::net::{BoxRecv, BoxSend, PeerId};
use crate::output::{say, short_peer_id};
use crate::route::{self, Route};

use super::super::outcome::ProxyOutcome;
use super::super::serve::ProxyConfig;
use super::{open_through_exits, OpenedUpstream, Upstream};

/// An open stream to the UDP gateway of an exit.
#[allow(dead_code)] // read by the association server
pub(in crate::proxy) struct ExitLink {
    pub route_label: String,
    pub peer: PeerId,
    pub send: BoxSend,
    pub recv: BoxRecv,
}

/// Why no UDP route opened: the ready outcome, and whether the exits have no UDP gateway (SOCKS reply 7, else 1).
#[allow(dead_code)] // read by the association server
pub(in crate::proxy) struct UdpRefusal {
    pub outcome: ProxyOutcome,
    pub no_udp: bool,
}

/// Where the datagrams of an association go.
#[allow(dead_code)] // read by the association server
pub(in crate::proxy) enum UdpRoute {
    Exit(ExitLink),
    Direct,
}

/// Chooses the route of a UDP association. It writes the `route` line.
#[allow(dead_code)] // read by the association server
pub(in crate::proxy) async fn open_udp_route(cfg: &ProxyConfig) -> Result<UdpRoute, UdpRefusal> {
    let route = match cfg.route.choose_now().await {
        Ok(route) => route,
        Err(e) => {
            say("udp", format!("{e:#}"));
            return Err(failure(ProxyOutcome::refused("no route", "the list of exits could not be read")));
        }
    };
    match route {
        Route::Direct => {
            say("route", "udp → direct");
            Ok(UdpRoute::Direct)
        }
        Route::Exits(candidates) => {
            say("route", format!("udp → exit {}", short_peer_id(&route::first_exit(&candidates))));
            open_exit_link(cfg, &candidates).await.map(UdpRoute::Exit)
        }
        Route::Refuse(reason) => {
            say("route", format!("udp → refused: {reason}"));
            Err(failure(ProxyOutcome::refused("no route", reason)))
        }
    }
}

/// Opens the gateway stream through the first candidate that has a UDP gateway.
#[allow(dead_code)] // read by the association server
pub(super) async fn open_exit_link(cfg: &ProxyConfig, candidates: &[PeerId]) -> Result<ExitLink, UdpRefusal> {
    let opened = open_through_exits(cfg, candidates, exit::UDPGW_HOST, exit::UDPGW_PORT).await;
    match opened {
        Ok(OpenedUpstream { route_label, upstream: Upstream::Exit { peer, send, recv } }) => {
            Ok(ExitLink { route_label, peer, send, recv })
        }
        Ok(other) => Err(failure(ProxyOutcome::refused(other.route_label, "no exit stream"))),
        Err(refused) => Err(UdpRefusal { no_udp: refused.gateway_refused, outcome: refused.outcome }),
    }
}

fn failure(outcome: ProxyOutcome) -> UdpRefusal {
    UdpRefusal { outcome, no_udp: false }
}
