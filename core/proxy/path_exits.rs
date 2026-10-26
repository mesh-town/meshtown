//! Opening a stream through one of the chosen exits: the candidate loop shared by TCP connections and the UDP gateway.
//! A child of `path`: it reuses the refusal type and `exit_refusal`.

use crate::exit::{self, PeerConnPool};
use crate::node::output::{say, short_peer_id};
use crate::transport::net::PeerId;

use super::super::outcome::ProxyOutcome;
use super::super::serve::ProxyConfig;
use super::{exit_refusal, ExitLink, OpenRefused};

/// What the candidates that did not open a stream left behind.
#[derive(Default)]
struct Misses {
    /// At least one `open_bi` timed out or an exit answered that it has no free slot (the exit is online but busy).
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
        match try_exit(cfg, peer, (target_text, port, gateway), &mut misses).await {
            Attempt::Opened(link) => return Ok(link),
            Attempt::Final(refusal) => return Err(refusal),
            Attempt::Next => {}
        }
    }
    Err(misses.into_refusal())
}

/// The result of asking one candidate exit.
enum Attempt {
    /// The exit opened the stream.
    Opened(ExitLink),
    /// The exit's answer ends the search.
    Final(OpenRefused),
    /// Try the next candidate.
    Next,
}

/// One attempt: asks `peer` to connect to `(target_text, port, is_gateway_target)`
/// and records in `misses` what a failed attempt leaves behind.
async fn try_exit(
    cfg: &ProxyConfig,
    peer: PeerId,
    (target_text, port, gateway): (&str, u16, bool),
    misses: &mut Misses,
) -> Attempt {
    let route_label = format!("exit {}", short_peer_id(&peer));
    if gateway && cfg.pool.gateway_refused_recently(peer) {
        misses.no_gateway = Some(remembered_gateway_refusal(peer, route_label));
        return Attempt::Next;
    }
    let (send, recv, code) =
        match exit::ask_exit_to_connect(&cfg.stack, &cfg.pool, peer, target_text, port).await {
            Ok(answer) => answer,
            Err(e) => {
                misses.busy |= is_busy(&e);
                misses.unanswered = true;
                note_failed_attempt(&cfg.pool, &peer, &e);
                return Attempt::Next;
            }
        };
    if code == exit::EXIT_CODE_OK {
        return Attempt::Opened(ExitLink {
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
        return Attempt::Final(OpenRefused::new(outcome));
    }
    misses.no_gateway = Some(OpenRefused::no_gateway(outcome));
    Attempt::Next
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

/// Logs a failed attempt. An exit's own "busy" reply is paced: a loaded exit answers it to a stream of requests.
fn note_failed_attempt(pool: &PeerConnPool, peer: &PeerId, e: &anyhow::Error) {
    let id = short_peer_id(peer);
    if e.downcast_ref::<exit::ExitBusy>().is_none() {
        say("route", format!("exit {id} did not answer: {e:#}"));
        return;
    }
    let Some(held) = pool.busy_line_due(std::time::Instant::now()) else {
        return;
    };
    say("route", busy_line(&id, held));
}

/// The client's line about an exit's "busy" reply; `held` further replies were not logged since the last line.
pub(super) fn busy_line(id: &str, held: u64) -> String {
    let tail = match held {
        0 => String::new(),
        n => format!(" (+{n} more, not logged)"),
    };
    format!("exit {id} is busy: no free stream slot{tail}")
}

/// The error says the exit is online but has no stream for us: the `open_bi` deadline or the exit's short reply.
fn is_busy(e: &anyhow::Error) -> bool {
    e.downcast_ref::<exit::StreamBusy>().is_some() || e.downcast_ref::<exit::ExitBusy>().is_some()
}

/// The refusal reason when no exit answered: `any_exit_stream_busy` — at least one exit is busy: the `open_bi`
/// deadline or the exit's short reply; otherwise the exits did not answer at all.
pub(super) fn no_answer_reason(any_exit_stream_busy: bool) -> &'static str {
    if any_exit_stream_busy {
        return "exit is busy (stream limit)";
    }
    "the selected exits did not answer"
}
