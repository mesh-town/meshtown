//! Tests for the node's live state (`telemetry.rs`).

use super::*;
use crate::transport::net::NodeSecret;

#[test]
fn rss_kib_on_current_os_is_not_zero() {
    let (cur, peak) = rss_kib();
    assert_ne!(cur, 0, "current RSS must be readable on this OS");
    assert_ne!(peak, 0, "peak RSS must be readable on this OS");
}

fn sample_id(byte: u8) -> PeerId {
    let secret = NodeSecret::from_bytes(&[byte; 32]);
    secret.peer_id()
}

#[test]
fn view_defaults_then_reflects_writes() {
    let tel = Telemetry::new();
    assert!(tel.view().dht_read.is_none() && TelemetryView::default().dht_read.is_none());
    assert!(tel.view().peers.is_empty());

    tel.set_dht_joined_and_size(true, 42);
    tel.note_publish();

    let id = sample_id(7);
    tel.peer_up(id, Some(PathKindView::Direct), Some(15));
    tel.peer_up(id, Some(PathKindView::Direct), Some(20)); // a repeated Up with the same id

    let view = tel.view();
    assert!(view.dht_joined);
    assert_eq!(view.dht_nodes, 42);
    assert!(view.record_published);
    assert!(view.last_publish.is_some());
    assert_eq!(
        view.peers.len(),
        1,
        "a repeated peer_up does not create a second record"
    );
    assert_eq!(view.peers[&id].rtt_ms, Some(20));
}
