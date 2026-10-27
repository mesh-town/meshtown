//! The route of a UDP association: direct, or a stream to the UDP gateway of an exit.
//! A child of `path`: it reuses `open_through_exits` and its refusal type.

use crate::exit;
use crate::node::output::{say, short_peer_id};
use crate::proxy::route;
use crate::transport::net::{BoxRecv, BoxSend, PeerId};

use super::super::outcome::ProxyOutcome;
use super::super::serve::ProxyConfig;
use super::{open_through_exits, OpenRefused};

/// An open stream to the UDP gateway of an exit.
pub(in crate::proxy) struct ExitLink {
    pub route_label: String,
    pub peer: PeerId,
    pub send: BoxSend,
    pub recv: BoxRecv,
}

/// Why no UDP route opened: the ready outcome, and whether the exits have no UDP gateway (SOCKS reply 7, else 1).
pub(in crate::proxy) struct UdpRefusal {
    pub outcome: ProxyOutcome,
    pub no_udp: bool,
}

/// Where the datagrams of an association go.
pub(in crate::proxy) enum UdpRoute {
    Exit(ExitLink),
    Direct,
}

/// Chooses the route of a UDP association. It writes the `route` line.
pub(in crate::proxy) async fn open_udp_route(cfg: &ProxyConfig) -> Result<UdpRoute, UdpRefusal> {
    let route = match cfg.route.choose_now().await {
        Ok(route) => route,
        Err(e) => {
            say("udp", format!("{e:#}"));
            return Err(failure(ProxyOutcome::refused(
                "no route",
                "the exit list could not be read",
            )));
        }
    };
    let candidates = match route.candidates() {
        Ok(candidates) => candidates,
        Err(reason) => {
            say("route", format!("udp → refused: {reason}"));
            return Err(failure(ProxyOutcome::refused("no route", reason)));
        }
    };
    let Some(candidates) = candidates else {
        say("route", "udp → direct");
        return Ok(UdpRoute::Direct);
    };
    say(
        "route",
        format!(
            "udp → exit {}",
            short_peer_id(&route::first_exit(&candidates))
        ),
    );
    open_exit_link(cfg, &candidates).await.map(UdpRoute::Exit)
}

/// Opens the gateway stream through the first candidate that has a UDP gateway.
pub(super) async fn open_exit_link(
    cfg: &ProxyConfig,
    candidates: &[PeerId],
) -> Result<ExitLink, UdpRefusal> {
    open_through_exits(cfg, candidates, exit::UDPGW_HOST, exit::UDPGW_PORT)
        .await
        .map_err(UdpRefusal::from)
}

impl From<OpenRefused> for UdpRefusal {
    fn from(refused: OpenRefused) -> Self {
        UdpRefusal {
            no_udp: refused.gateway_refused,
            outcome: refused.outcome,
        }
    }
}

fn failure(outcome: ProxyOutcome) -> UdpRefusal {
    UdpRefusal {
        outcome,
        no_udp: false,
    }
}
