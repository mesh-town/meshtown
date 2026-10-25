//! Opening a stream through one of the chosen exits: the candidate loop shared by TCP connections and the UDP gateway.
//! A child of `path`: it reuses the refusal type and `exit_refusal`.

use crate::exit;
use crate::node::output::{say, short_peer_id};
use crate::transport::net::PeerId;

use super::super::outcome::ProxyOutcome;
use super::super::serve::ProxyConfig;
use super::{exit_refusal, ExitLink, OpenRefused};

/// What the candidates that did not open a stream left behind.
#[derive(Default)]
struct Misses {
    /// At least one `open_bi` timed out (the exit is online but its streams are busy).
    busy: bool,
    /// At least one candidate did not answer at all.
    unanswered: bool,
    /// The last refusal of the UDP gateway target.
    no_gateway: Option<OpenRefused>,
}

impl Misses {
    /// The final refusal after every candidate failed. "No UDP gateway" only if every candidate refused the
    /// gateway: one that did not answer may still have it.
    fn into_refusal(self) -> OpenRefused {
        match self.no_gateway {
            Some(refusal) if !self.unanswered => refusal,
            _ => OpenRefused::new(ProxyOutcome::refused(
                "no route",
                no_answer_reason(self.busy),
            )),
        }
    }
}

/// The route through one of the chosen exits: tries the candidates in order, moving to the
/// next one on a transport error and, for the UDP gateway target, on a refusal of the gateway; any other
/// exit reply code is final. When every candidate refused the gateway, the last such refusal is returned.
pub(super) async fn open_through_exits(
    cfg: &ProxyConfig,
    candidates: &[PeerId],
    target_text: &str,
    port: u16,
) -> Result<ExitLink, OpenRefused> {
    let mut misses = Misses::default();
    let gateway = exit::is_gateway_target(target_text, port);
    for &peer in candidates {
        let route_label = format!("exit {}", short_peer_id(&peer));
        if gateway && cfg.pool.gateway_refused_recently(peer) {
            misses.no_gateway = Some(remembered_gateway_refusal(peer, route_label));
            continue;
        }
        let (send, recv, code) =
            match exit::ask_exit_to_connect(&cfg.stack, &cfg.pool, peer, target_text, port).await {
                Ok(answer) => answer,
                Err(e) => {
                    misses.busy |= e.downcast_ref::<exit::StreamBusy>().is_some();
                    misses.unanswered = true;
                    say(
                        "route",
                        format!("exit {} did not answer: {e:#}", short_peer_id(&peer)),
                    );
                    continue;
                }
            };
        if code == exit::EXIT_CODE_OK {
            return Ok(ExitLink {
                route_label,
                peer,
                send,
                recv,
            });
        }
        let gateway_refusal = gateway && code == exit::EXIT_CODE_ADDRESS_FORBIDDEN;
        if gateway_refusal {
            cfg.pool.note_gateway_refused(peer);
        }
        let outcome = exit_refusal(peer, code, route_label);
        if !gateway_refusal {
            return Err(OpenRefused::new(outcome));
        }
        misses.no_gateway = Some(OpenRefused::no_gateway(outcome));
    }
    Err(misses.into_refusal())
}

/// The refusal for the UDP gateway target on an exit that already refused it: the exit is not asked again.
fn remembered_gateway_refusal(peer: PeerId, route_label: String) -> OpenRefused {
    let code = exit::EXIT_CODE_ADDRESS_FORBIDDEN;
    say(
        "exit",
        format!("{}: no UDP gateway (refused earlier)", short_peer_id(&peer)),
    );
    let reason = format!("code {code} — {}", exit::exit_code_text(code));
    OpenRefused::no_gateway(ProxyOutcome::refused(route_label, reason))
}

/// The refusal reason when no exit answered: `any_exit_stream_busy` — at least one `open_bi` timed out
/// (the exit is online but its streams are busy), otherwise the exits did not answer at all.
pub(super) fn no_answer_reason(any_exit_stream_busy: bool) -> &'static str {
    if any_exit_stream_busy {
        return "exit is busy (stream limit)";
    }
    "the selected exits did not answer"
}
