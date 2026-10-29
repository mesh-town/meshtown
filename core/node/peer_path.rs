//! A peer's path for the node: recorded when a connection appears, and watching for changes of the chosen path.
//!
//! What it does: `record_peer_up` puts the chosen path's hint into the stack's discovery memory and the kind of
//! path into telemetry; `watch_path` prints `[path]` and updates telemetry on every path change.
//! What it doesn't do: doesn't know which transport is under the connection — takes everything from the `net` facade
//! (`PeerConn::link`, `PeerConn::path_watch`); doesn't open or close connections.

use std::sync::Arc;
use std::time::Duration;

use crate::members::membership::Members;
use crate::node::output::{say, short_peer_id};
use crate::node::telemetry::Telemetry;
use crate::transport::net::{
    AddrHint, NetStack, PathKind, PathWatch, PeerAddr, PeerConn, PeerId, RelayRef,
};
use crate::web_gui::PathKindView;

fn rtt_ms(rtt: Option<Duration>) -> Option<u64> {
    rtt.map(|d| d.as_millis() as u64)
}

/// Records the chosen path of a new connection: the hint goes into the stack's discovery memory, the kind of path
/// and the latency into telemetry; returns the hint (for the peer cache), if there is one.
pub(crate) fn record_peer_up(
    conn: &dyn PeerConn,
    stack: &NetStack,
    tel: &Telemetry,
) -> Option<AddrHint> {
    let peer = conn.peer();
    let link = conn.link();
    let hint = link.hint();
    if let Some(h) = &hint {
        stack.add_hints(PeerAddr {
            id: peer,
            hints: vec![h.clone()],
        });
    }
    tel.peer_up(peer, PathKindView::from_net(&link.path), rtt_ms(link.rtt));
    hint
}

/// The log line for a connection that runs through a relay; `None` for any other path.
fn relay_connect_line(
    who: &str,
    path: &PathKind,
    label: impl Fn(PeerId) -> String,
) -> Option<String> {
    let PathKind::Relayed(relay) = path else {
        return None;
    };
    let via = match relay {
        RelayRef::Peer(id) => label(*id),
        RelayRef::Url(url) => url.clone(),
        RelayRef::Unnamed => "a relay server".to_string(),
    };
    Some(format!("connected to {who} via relay of {via}"))
}

/// Prints a `[relay]` line when the new connection runs through a relay.
pub(crate) fn say_if_relayed(conn: &dyn PeerConn, members: &Members) {
    let who = members.peer_label(conn.peer());
    let line = relay_connect_line(&who, &conn.link().path, |id| members.peer_label(id));
    if let Some(line) = line {
        say("relay", line);
    }
}

/// Prints a line on every change of the chosen path until the connection closes and writes the same path
/// into the node's telemetry.
pub(crate) async fn watch_path(mut watch: Box<dyn PathWatch>, peer: PeerId, tel: Arc<Telemetry>) {
    while let Some(ch) = watch.next().await {
        say("path", format!("{}: {}", short_peer_id(&peer), ch.log_line));
        tel.record_peer_path(peer, PathKindView::from_net(&ch.path), rtt_ms(ch.rtt));
    }
}

#[cfg(test)]
#[path = "peer_path_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "peer_path_relay_tests.rs"]
mod relay_tests;

#[cfg(test)]
#[path = "peer_path_hint_tests.rs"]
mod hint_tests;
