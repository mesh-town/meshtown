//! Tests of `node/peer_path.rs`: `record_peer_up` (the path hint goes into the transport's discovery memory, the kind of
//! path and rtt into telemetry) and `watch_path` (the `[path]` line and telemetry on every path
//! change event). A connection without paths comes from `mem`,
//! the path change events come from a fake `PathWatch` with a preset list.

use crate::net::contract::{pid, secret, must_finish, SEED_A, SEED_B};
use crate::net::facade::test_support::Spy;
use crate::net::{
    BoxFuture, MemNet, PathChange, PathKind as NetPath, PathWatch, PeerConn, PeerId,
    SplitTransport, Transport, AUTH,
};
use crate::output::{log_page, log_summary, short_peer_id};
use crate::telemetry::Telemetry;
use crate::web::PathKindView;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use super::{record_peer_up, watch_path};

/// Seeds of the peers of fake streams: no other test takes them, so a short id in the
/// shared log unambiguously points at the lines of this very test.
const TWO_EVENTS_SEED: u8 = 0x51;
const ONE_EVENT_SEED: u8 = 0x52;
const NO_EVENTS_SEED: u8 = 0x53;

mod new_connection {
    use super::*;

    #[tokio::test]
    async fn record_peer_up_on_mem_gives_no_hint() {
        // given: a mem connection (no socket address) and a spy transport that remembers hints
        let fx = setup_mem().await;
        let peer = fx.conn.peer();
        let tel = Telemetry::new();
        // when
        let stack = crate::net::NetStackBuilder::new(secret(SEED_A)).transport(fx.spy.clone()).build().unwrap();
        let hint = record_peer_up(&*fx.conn, &stack, &tel);
        // then: no hint, none was passed to the transport
        assert!(hint.is_none(), "a hint for mem: {hint:?}");
        assert!(fx.spy.hints().is_empty(), "the transport got hints: {:?}", fx.spy.hints());
        // then: the member is still recorded as "online"
        let view = tel.view();
        let live = view.peers.get(&peer).expect("the member is not recorded in telemetry");
        assert!(live.online, "the member is not marked online");
        fx.close().await;
    }
}

mod path_events {
    use super::*;

    #[tokio::test]
    async fn watch_path_says_peer_and_text() {
        // given: a stream of two events — a direct path with rtt 12 ms, then "no chosen path"
        let peer = pid(TWO_EVENTS_SEED);
        let tel = Telemetry::new();
        let before = log_summary(0).last_seq;
        let watch = scripted(vec![direct_12ms(), no_path()]);
        // when: watch_path runs to the end of the stream
        must_finish(watch_path(watch, peer, tel.clone())).await;
        // then: the log has two `[path]` lines — the peer and the event text verbatim, in order
        let short_id = short_peer_id(&peer);
        assert_eq!(
            path_lines_about(before, peer),
            vec![
                format!("{short_id}: direct 1.2.3.4:5, rtt 12 ms"),
                format!("{short_id}: no chosen path"),
            ],
        );
        // then: telemetry repeats the last event — no path and no rtt
        assert_eq!(path_and_rtt(&tel, peer), (None, None), "telemetry after the second event");
    }

    #[tokio::test]
    async fn watch_path_writes_event_path_and_rtt_to_telemetry() {
        // given: a stream of one event — a direct path with rtt 12 ms
        let peer = pid(ONE_EVENT_SEED);
        let tel = Telemetry::new();
        let watch = scripted(vec![direct_12ms()]);
        // when
        must_finish(watch_path(watch, peer, tel.clone())).await;
        // then: telemetry has path Direct and rtt 12 ms
        assert_eq!(
            path_and_rtt(&tel, peer),
            (Some(PathKindView::Direct), Some(12)),
            "telemetry after the first event"
        );
    }

    #[tokio::test]
    async fn watch_path_ends_on_none() {
        // given: a stream that ended at once
        let peer = pid(NO_EVENTS_SEED);
        let tel = Telemetry::new();
        let before = log_summary(0).last_seq;
        // when: watch_path is called
        must_finish(watch_path(scripted(Vec::new()), peer, tel.clone())).await;
        // then: it returned; the log and telemetry are untouched
        let lines = path_lines_about(before, peer);
        assert!(lines.is_empty(), "log lines without events: {lines:?}");
        assert!(tel.view().peers.is_empty(), "telemetry got a record without events");
    }
}

// ---- helpers ----

/// Fake `PathWatch`: hands out the given events in order, then `None`.
struct Scripted(VecDeque<PathChange>);

impl PathWatch for Scripted {
    fn next(&mut self) -> BoxFuture<'_, Option<PathChange>> {
        let event = self.0.pop_front();
        Box::pin(async move { event })
    }
}

fn scripted(events: Vec<PathChange>) -> Box<dyn PathWatch> {
    Box::new(Scripted(events.into()))
}

/// Event "direct path 1.2.3.4:5, rtt 12 ms".
fn direct_12ms() -> PathChange {
    PathChange {
        path: NetPath::Direct,
        rtt: Some(Duration::from_millis(12)),
        log_line: "direct 1.2.3.4:5, rtt 12 ms".to_string(),
    }
}

/// Event "no chosen path".
fn no_path() -> PathChange {
    PathChange {
        path: NetPath::Unknown,
        rtt: None,
        log_line: "no chosen path".to_string(),
    }
}

/// A member's path and rtt in telemetry; no such member — panic.
fn path_and_rtt(tel: &Telemetry, peer: PeerId) -> (Option<PathKindView>, Option<u64>) {
    let view = tel.view();
    let live = view.peers.get(&peer).expect("the member is not recorded in telemetry");
    (live.path, live.rtt_ms)
}

/// Texts of the `[path]` lines printed after `after` (the log's `last_seq` before the call) that
/// start with the short id of `peer`.
fn path_lines_about(after: u64, peer: PeerId) -> Vec<String> {
    let prefix = format!("{}: ", short_peer_id(&peer));
    let page = log_page(after, 10_000);
    let ours = page
        .lines
        .into_iter()
        .filter(|line| line.tag == "path" && line.text.starts_with(&prefix));
    ours.map(|line| line.text).collect()
}

/// A mem connection and spy transport A, which the hint is addressed to.
struct MemFixture {
    spy: Arc<Spy>,
    conn: Arc<dyn PeerConn>,
    far: Arc<SplitTransport>,
    /// The accepted side lives until the end of the test: the last handle would close the connection.
    _accepted: Arc<dyn PeerConn>,
}

async fn setup_mem() -> MemFixture {
    let net = MemNet::new();
    let spy = Spy::on(&net, &secret(SEED_A)).shared();
    let far = net.transport(&secret(SEED_B));
    far.serve(&[AUTH]);
    let (dialed, accepted) = tokio::join!(must_finish(spy.connect(&far.local_addr(), AUTH)), must_finish(far.accept()));
    let conn = dialed.expect("connect mem gave no connection");
    let (_, accepted) = accepted.expect("accept mem closed");
    MemFixture { spy, conn, far, _accepted: accepted }
}

impl MemFixture {
    async fn close(self) {
        must_finish(self.spy.close()).await;
        must_finish(self.far.close()).await;
    }
}
