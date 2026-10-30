//! The path watch of a relayed session: it reports the move from the relay to the direct path.

use super::*;
use crate::transport::net::link::{PathChange, PathKind};
use crate::transport::net::transport::PathWatch;
use std::time::Duration;
use tokio::sync::watch;
use townjump_dht::RelayPath;

/// How long a watch that must answer at once may take.
const PROMPT: Duration = Duration::from_secs(2);
/// How long a watch that must stay silent is observed.
const SILENT: Duration = Duration::from_millis(200);

mod relayed_session {
    use super::*;

    mod switches_to_direct {
        use super::*;

        #[tokio::test]
        async fn switch_to_direct_is_reported_once() {
            // given: a session on a relay, whose source stays open
            let (tx, mut watch) = setup(RelayPath::Relayed);
            // when: the connection moves to the direct path
            tx.send(RelayPath::Direct).unwrap();
            let first = next_within(&mut watch, PROMPT).await;
            // then: one change to the direct path, with an unknown rtt and a log line
            let change = first.expect("the switch must be reported");
            assert!(
                matches!(change.path, PathKind::Direct),
                "the path must be direct"
            );
            assert!(change.rtt.is_none(), "the watch has no rtt of its own");
            assert!(
                change.log_line.contains("direct"),
                "the log line names the direct path: {}",
                change.log_line
            );
            // and: nothing more is reported while the source stays open
            assert!(
                next_within(&mut watch, SILENT).await.is_none(),
                "the switch is reported once"
            );
        }
    }

    mod stays_on_the_relay {
        use super::*;

        #[tokio::test]
        async fn nothing_is_reported_while_relayed() {
            // given: a session on a relay, whose source stays open
            let (_tx, mut watch) = setup(RelayPath::Relayed);
            // when
            let change = next_within(&mut watch, SILENT).await;
            // then
            assert!(
                change.is_none(),
                "a relayed session has no change to report"
            );
        }
    }

    mod source_closes {
        use super::*;

        #[tokio::test]
        async fn closed_source_ends_the_watch() {
            // given: a session on a relay
            let (tx, mut watch) = setup(RelayPath::Relayed);
            // when: the source goes away without a switch
            drop(tx);
            // then: the watch ends, and stays ended
            assert!(
                ends_within(&mut watch, PROMPT).await,
                "a closed source must end the watch"
            );
            assert!(
                ends_within(&mut watch, PROMPT).await,
                "an ended watch stays ended"
            );
        }
    }
}

mod direct_session {
    use super::*;

    #[tokio::test]
    async fn already_direct_is_reported() {
        // given: the source already says `Direct` when the watch starts
        let (_tx, mut watch) = setup(RelayPath::Direct);
        // when
        let change = next_within(&mut watch, PROMPT).await;
        // then: the move is reported at once, not lost
        let change = change.expect("a path that is already direct must be reported");
        assert!(
            matches!(change.path, PathKind::Direct),
            "the path must be direct"
        );
    }
}

mod session_without_relay {
    use super::*;

    #[tokio::test]
    async fn no_relay_path_ends_at_once() {
        // given: a session that was not set up through a relay has no path source
        let mut watch = RelayPathWatch::new(None);
        // when / then
        assert!(
            ends_within(&mut watch, PROMPT).await,
            "no source: the watch must end at once"
        );
        assert!(
            ends_within(&mut watch, PROMPT).await,
            "an ended watch stays ended"
        );
    }
}

/// A watch over a source that starts at `initial`; the sender keeps the source open.
fn setup(initial: RelayPath) -> (watch::Sender<RelayPath>, RelayPathWatch) {
    let (tx, rx) = watch::channel(initial);
    (tx, RelayPathWatch::new(Some(rx)))
}

/// The next change, or `None` when the watch ends or says nothing within `limit`.
async fn next_within(watch: &mut RelayPathWatch, limit: Duration) -> Option<PathChange> {
    tokio::time::timeout(limit, watch.next())
        .await
        .ok()
        .flatten()
}

/// Whether `next` answers `None` within `limit` (silence is not an end).
async fn ends_within(watch: &mut RelayPathWatch, limit: Duration) -> bool {
    matches!(tokio::time::timeout(limit, watch.next()).await, Ok(None))
}
