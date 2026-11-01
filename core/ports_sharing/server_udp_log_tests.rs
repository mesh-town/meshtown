//! Tests of the thinned "UDP port opened" log line.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::members::membership::Members;
use crate::ports_sharing::PublishedPortsTable;
use crate::transport::net::facade::test_support::pid;
use tokio::sync::Semaphore;

fn server() -> PortServer {
    PortServer {
        members: Members::default(),
        table: PublishedPortsTable::new(vec![]),
        udp_sessions: Arc::new(Semaphore::new(1)),
        udp_lines: Arc::default(),
        traffic: Default::default(),
    }
}

#[test]
fn udp_opened_text_has_no_tail_when_nothing_was_held() {
    assert_eq!(udp_opened_text("A", 53, 0), "A: port 53/udp opened");
    assert_eq!(
        udp_opened_text("A", 53, 2),
        "A: port 53/udp opened (+2 more, not logged)"
    );
}

#[test]
fn udp_opened_line_is_thinned_per_peer_and_port() {
    // given: three sessions of one member on one port in a row
    let server = server();
    let (peer, t0) = (pid(1), Instant::now());
    let lines: Vec<_> = (0..3)
        .map(|_| udp_opened_line(&server, peer, 53, t0))
        .collect();

    // then: only the first got a line
    assert!(lines[0].is_some());
    assert!(lines[1..].iter().all(Option::is_none));

    // when: the window has passed, the next line reports the two held back
    let later = t0 + Duration::from_secs(11);
    assert_eq!(server.udp_lines.due((peer, 53), later), Some(2));
    // another port or another member has its own first line
    assert_eq!(server.udp_lines.due((peer, 54), later), Some(0));
    assert_eq!(server.udp_lines.due((pid(2), 53), later), Some(0));
}
