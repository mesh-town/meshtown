//! Tests of the `mem` connection (continuation of `conn_tests.rs`): `PeerConn` diagnostics of `mem` — empty by default.

use super::*;

#[tokio::test]
async fn mem_conn_diag_defaults_are_empty() {
    // given: a fresh pair of mem connections
    let (a, _b) = conns();
    // when: we ask for texts, paths, link() and take the next path_watch() event
    let (path, congestion, reason) = (a.path_text(), a.congestion_text(), a.close_reason());
    let lines = a.path_lines();
    let link = a.link();
    let mut watch = a.path_watch();
    let event = within("path_watch().next()", watch.next()).await;
    // then: texts and lines are empty, the path and frame counters are zero, there are no events
    assert_eq!(path, "", "path_text of mem");
    assert_eq!(congestion, "", "congestion_text of mem");
    assert_eq!(reason, "", "close_reason of a live mem connection");
    assert!(lines.is_empty(), "path_lines of mem: {lines:?}");
    assert_eq!(link.path_count, 0, "path_count of mem");
    assert_eq!(link.stream_frames, 0, "stream_frames of mem");
    assert!(
        event.is_none(),
        "path_watch of mem returned an event: {event:?}"
    );
}
