//! Tests for filtering out departed members (7 days without news) in `peers[]` and for `last_seen_sec` as the minimum over
//! the sources. The helpers come from `super::tests`.

use super::tests::{base, pair, record_at, view};
use super::*;

const T: u64 = 1_000_000_000;
const D: u64 = 86_400;

fn inputs_with_cache(age: u64) -> (Inputs<'static>, PeerId) {
    let my_id = pair(1);
    let x = pair(2);
    let mut inputs = base(my_id);
    inputs.now_unix = T;
    inputs.cached_seen = [(x, T - age)].into_iter().collect();
    (inputs, x)
}

fn has(peers: &[PeerInfo], id: PeerId) -> bool {
    peers.iter().any(|p| p.id_full == id.to_string())
}

fn row(peers: &[PeerInfo], id: PeerId) -> PeerInfo {
    peers.iter().find(|p| p.id_full == id.to_string()).unwrap().clone()
}

#[test]
fn cache_eight_days_hidden() {
    let (inputs, x) = inputs_with_cache(8 * D);
    assert!(!has(&build_peers(&inputs), x));
}

#[test]
fn cache_six_days_shown() {
    let (inputs, x) = inputs_with_cache(6 * D);
    assert!(has(&build_peers(&inputs), x));
}

#[test]
fn exactly_seven_days_shown() {
    let (inputs, x) = inputs_with_cache(7 * D);
    assert!(has(&build_peers(&inputs), x));
}

#[test]
fn returned_by_fresh_check_shown() {
    let (mut inputs, x) = inputs_with_cache(8 * D);
    inputs.domain_names = vec!["a.example".to_string()];
    inputs.check_views = vec![view("a.example", x, T, Some(record_at(CheckVerdict::Ok, T - 60)))];
    assert!(has(&build_peers(&inputs), x));
}

#[test]
fn returned_by_doc_seen_shown() {
    let (mut inputs, x) = inputs_with_cache(8 * D);
    inputs.doc_seen = [(x, T - 3600)].into_iter().collect();
    assert!(has(&build_peers(&inputs), x));
}

#[test]
fn returned_by_member_shown_online() {
    let (mut inputs, x) = inputs_with_cache(8 * D);
    inputs.members = [x].into_iter().collect();
    let peers = build_peers(&inputs);
    assert!(row(&peers, x).online);
}

#[test]
fn returned_by_fresh_cache_shown() {
    let (mut inputs, x) = inputs_with_cache(8 * D);
    inputs.cached_seen = [(x, T - 60)].into_iter().collect();
    assert!(has(&build_peers(&inputs), x));
}

#[test]
fn chosen_exit_with_old_cache_shown() {
    let (mut inputs, x) = inputs_with_cache(8 * D);
    inputs.chosen_exits = vec![x];
    assert!(has(&build_peers(&inputs), x));
}

#[test]
fn self_row_always_first() {
    let (inputs, _) = inputs_with_cache(8 * D);
    let peers = build_peers(&inputs);
    assert!(peers[0].is_self);
    assert_eq!(peers[0].id_full, inputs.id.to_string());
}

#[test]
fn static_cache_six_days_gone_two_days_later() {
    let (mut inputs, x) = inputs_with_cache(6 * D);
    assert!(has(&build_peers(&inputs), x));
    inputs.now_unix = T + 2 * D;
    assert!(!has(&build_peers(&inputs), x));
}

#[test]
fn last_seen_equals_age() {
    let (mut inputs, x) = inputs_with_cache(3 * D);
    inputs.doc_seen = [(x, T - D)].into_iter().collect();
    assert_eq!(row(&build_peers(&inputs), x).last_seen_sec, D);
}
