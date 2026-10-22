//! Session close log line: the text is built by a pure function and compared as a string.

use super::*;
use crate::transport::net::id::NodeSecret;

fn peer() -> PeerId {
    NodeSecret::from_bytes(&[3; 32]).peer_id()
}

#[test]
fn close_line_names_peer_session_side_code_reason() {
    let id = peer();
    let head = id.to_string()[..8].to_string();
    let us = close_line(&id, 0xABCD_0001, Side::Us, 7, b"bye");
    assert_eq!(
        us,
        format!("session {head} #abcd0001 closed by us: code 7: bye")
    );
    let them = close_line(&id, 0xABCD_0001, Side::Peer, 0, "read ended".as_bytes());
    assert_eq!(
        them,
        format!("session {head} #abcd0001 closed not by us: code 0: read ended")
    );
}

#[test]
fn close_line_survives_non_utf8_reason() {
    let line = close_line(&peer(), 1, Side::Peer, 2, &[0xFF, b'a']);
    assert!(line.ends_with("code 2: \u{FFFD}a"), "{line}");
}
