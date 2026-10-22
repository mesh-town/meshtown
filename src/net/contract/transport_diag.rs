//! Contract suite for connection diagnostics: the `transport_diag_contract!` macro and the case helpers.
//! What it checks: `path_watch()` ends when the connection closes, path-change events carry text,
//! and the text methods of `PeerConn` and `link()` do not fail on a closed connection.
//! Plugging in an adapter: `transport_diag_contract!(fn_without_arguments_returning_impl_Harness);`.

use crate::net::contract::transport_cases::{connect_between, transport_pair, ping_pong};
use crate::net::contract::{Harness, STEP};
use crate::net::link::PathChange;
use crate::net::service::AUTH;
use crate::net::transport::{PathWatch, PeerConn, Transport};
use std::sync::Arc;
use std::time::Duration;

/// How long we wait for the event that the watcher returns while the connection is alive (iroh returns the current snapshot
/// at once, the other transports return `None` or stay silent).
const LIVE_PROBE: Duration = Duration::from_millis(500);

/// A live connection A→B after the "ping"/"pong" exchange, together with what keeps it alive.
/// The fields `_a`, `_b` are not read: dropping either of them would close `conn`.
pub(crate) struct ExchangedPair {
    pub _a: Arc<dyn Transport>,
    pub _b: Arc<dyn Transport>,
    /// Handle of the dialer (A→B): the case asks it for diagnostics.
    pub conn: Arc<dyn PeerConn>,
    /// Handle of the acceptor (at B).
    pub accepted: Arc<dyn PeerConn>,
}

/// Two nodes of the environment, a connection A→B on AUTH and a "ping"/"pong" exchange over it (modelled on `link_after_exchange`).
pub(crate) async fn exchanged_pair(h: &impl Harness) -> ExchangedPair {
    let (a, b) = transport_pair(h).await;
    let (_, pair) = connect_between(&a, &b, AUTH).await;
    ping_pong(&pair).await;
    ExchangedPair {
        _a: a,
        _b: b,
        conn: pair.a,
        accepted: pair.b,
    }
}

/// Reads the watcher until `None`; `None` in the answer means no `None` came from the watcher within `STEP`.
pub(crate) async fn drain_until_none(watch: &mut dyn PathWatch) -> Option<Vec<PathChange>> {
    let draining = async {
        let mut events = Vec::new();
        while let Some(change) = watch.next().await {
            events.push(change);
        }
        events
    };
    tokio::time::timeout(STEP, draining).await.ok()
}

/// The event that the watcher returns while the connection is alive; `None` means the watcher has ended or is silent.
pub(crate) async fn event_while_alive(watch: &mut dyn PathWatch) -> Option<PathChange> {
    let waited = tokio::time::timeout(LIVE_PROBE, watch.next()).await;
    waited.ok().flatten()
}

macro_rules! transport_diag_contract {
    ($harness:path) => {
        mod transport_diag {
            use super::*;
            use $crate::net::contract::must_finish;
            use $crate::net::contract::transport_diag::{
                drain_until_none, event_while_alive, exchanged_pair,
            };

            mod after_exchange {
                use super::*;

                mod closed {
                    use super::*;

                    #[tokio::test]
                    async fn path_watch_ends_after_close() {
                        // given: A and B exchanged "ping" and "pong", A took path_watch()
                        let h = $harness();
                        let pair = exchanged_pair(&h).await;
                        let mut watch = pair.conn.path_watch();
                        // when: A closes the connection
                        pair.conn.close(7, b"bye");
                        // then: next() reaches None within 5 s (not necessarily on the first next():
                        // the closed flag in the iroh path snapshot rises later than close)
                        let ended = drain_until_none(&mut *watch).await;
                        assert!(
                            ended.is_some(),
                            "path_watch did not return None within 5 s after close"
                        );
                    }

                    #[tokio::test]
                    async fn diag_calls_survive_closed_conn() {
                        // given: the connection is closed by A, both handles see it as closed
                        let h = $harness();
                        let pair = exchanged_pair(&h).await;
                        pair.conn.close(7, b"bye");
                        must_finish(pair.conn.closed()).await;
                        must_finish(pair.accepted.closed()).await;
                        // when: we ask each handle for texts and paths and link(), and take a new path_watch
                        for conn in [&pair.conn, &pair.accepted] {
                            let _ = (
                                conn.path_text(),
                                conn.congestion_text(),
                                conn.close_reason(),
                                conn.link(),
                            );
                            let lines = conn.path_lines();
                            let mut watch = conn.path_watch();
                            let ended = drain_until_none(&mut *watch).await;
                            // then: no panic; the path lines are non-empty; the new path_watch also ends
                            assert!(
                                lines.iter().all(|l| !l.is_empty()),
                                "empty path line: {lines:?}"
                            );
                            assert!(
                                ended.is_some(),
                                "path_watch of the closed connection did not return None within 5 s"
                            );
                        }
                    }
                }

                mod alive_until_close {
                    use super::*;

                    #[tokio::test]
                    async fn path_watch_events_have_text() {
                        // given: the connection is alive, A took path_watch(), the event of the live connection (if any) was consumed
                        let h = $harness();
                        let pair = exchanged_pair(&h).await;
                        let mut watch = pair.conn.path_watch();
                        let mut events: Vec<_> =
                            event_while_alive(&mut *watch).await.into_iter().collect();
                        // when: A closes the connection and the events are collected up to None
                        pair.conn.close(7, b"bye");
                        let rest = drain_until_none(&mut *watch).await;
                        events.extend(rest.expect("path_watch did not return None within 5 s after close"));
                        // then: every event has non-empty text
                        let blank = events.iter().filter(|e| e.log_line.is_empty()).count();
                        assert_eq!(blank, 0, "events without text: {events:?}");
                    }
                }
            }
        }
    };
}

pub(crate) use transport_diag_contract;
