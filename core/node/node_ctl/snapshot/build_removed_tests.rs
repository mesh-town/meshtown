//! Tests for hiding removed nodes (forgotten, or forgotten by this node's own pending vote) in `peers[]`.
//! The helpers come from `super::tests`.

use super::tests::{base, pair, record_at, view};
use super::*;
use crate::node::telemetry::PeerLive;
use std::time::Instant;

const T: u64 = 1_000_000_000;

fn has(peers: &[PeerInfo], id: PeerId) -> bool {
    peers.iter().any(|p| p.id_full == id.to_string())
}

/// Inputs at time `T` where `x` is removed and has a fresh cache entry.
fn removed_cached() -> (Inputs<'static>, PeerId) {
    let x = pair(2);
    let mut inputs = base(pair(1));
    inputs.now_unix = T;
    inputs.cached_seen = [(x, T)].into_iter().collect();
    inputs.removed = [x].into_iter().collect();
    (inputs, x)
}

#[test]
fn forgotten_cached_peer_hidden() {
    let (inputs, x) = removed_cached();
    assert!(!has(&build_peers(&inputs), x));
}

#[test]
fn forgotten_member_hidden() {
    let (mut inputs, x) = removed_cached();
    inputs.members.insert(x);
    assert!(!has(&build_peers(&inputs), x));
}

#[test]
fn forgotten_telemetry_peer_hidden() {
    let (mut inputs, x) = removed_cached();
    let live = PeerLive {
        online: true,
        path: None,
        rtt_ms: None,
        last_seen: Instant::now(),
    };
    inputs.telemetry.peers.insert(x, live);
    assert!(!has(&build_peers(&inputs), x));
}

#[test]
fn forgotten_with_fresh_check_hidden() {
    let (mut inputs, x) = removed_cached();
    inputs.domain_names = vec!["a.example".to_string()];
    let rec = Some(record_at(CheckVerdict::Ok, T));
    inputs.check_views = vec![view("a.example", x, T, rec)];
    assert!(!has(&build_peers(&inputs), x));
}

#[test]
fn forgotten_chosen_exit_kept() {
    let (mut inputs, x) = removed_cached();
    inputs.chosen_exits = vec![x];
    assert!(has(&build_peers(&inputs), x));
}

#[test]
fn removed_self_row_stays_first() {
    let (mut inputs, x) = removed_cached();
    inputs.removed.insert(inputs.id);
    let peers = build_peers(&inputs);
    assert!(peers[0].is_self);
    assert!(!has(&peers, x));
}

#[test]
fn others_kept_next_to_hidden() {
    let (mut inputs, x) = removed_cached();
    let (a, b) = (pair(3), pair(4));
    inputs.cached_seen.extend([(a, T), (b, T)]);
    let ids: Vec<String> = build_peers(&inputs)
        .iter()
        .skip(1)
        .map(|p| p.id_full.clone())
        .collect();
    let mut want = [a, b];
    want.sort_by_key(output::short_peer_id);
    assert_eq!(ids, want.iter().map(|p| p.to_string()).collect::<Vec<_>>());
    assert!(!ids.contains(&x.to_string()));
}

#[test]
fn restored_peer_shown_again() {
    let (mut inputs, x) = removed_cached();
    inputs.removed.clear();
    assert!(has(&build_peers(&inputs), x));
}
