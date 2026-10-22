//! Tests of handling the main loop's wake-ups (`event_loop_wake.rs`) and the member lines.
//! No sockets: the whole loop needs a stack.

use super::down_line;
use super::up_line;
use super::wake::{failed_wake, found_wake, member_wake, Wake};
use crate::membership::MemberEvent;
use crate::net::contract::pid;

#[test]
fn closed_found_channel_stops_loop() {
    assert!(matches!(found_wake(None), Wake::Closed));
}

#[test]
fn found_addr_goes_to_dialer() {
    let addr = crate::net::PeerAddr::new(pid(1));
    assert!(matches!(found_wake(Some(addr)), Wake::Found(a) if a.id == pid(1)));
}

#[test]
fn closed_event_channel_stops_loop() {
    assert!(matches!(member_wake(None), Wake::Closed));
}

#[test]
fn member_down_passes_through() {
    let event = MemberEvent::Down {
        peer: pid(2),
        reason: "timeout".to_string(),
    };
    assert!(
        matches!(member_wake(Some(event)), Wake::Member(MemberEvent::Down { peer, .. }) if peer == pid(2))
    );
}

/// A closed dial failure channel doesn't stop the loop.
#[test]
fn closed_failed_channel_does_not_stop() {
    assert!(matches!(failed_wake(None), Wake::Idle));
}

#[test]
fn up_line_text() {
    assert_eq!(up_line("peer", 3), "+ peer (peers: 3)");
}

#[test]
fn down_line_text() {
    assert_eq!(
        down_line("peer", "timeout", 2),
        "- peer: timeout (peers: 2)"
    );
}
