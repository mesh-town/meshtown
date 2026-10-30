use super::*;
use crate::members::network_key::NetworkKey;
use std::sync::atomic::Ordering;

fn exits() -> (watch::Receiver<Vec<PeerId>>, bool) {
    (watch::channel(Vec::new()).1, false)
}

#[test]
fn zero_start_interval_becomes_one_second() {
    let probe = new_probe_settings(Duration::ZERO, 0, exits());
    assert_eq!(probe.interval_secs.load(Ordering::SeqCst), 1);
    assert_eq!(probe.ok_probe_count.load(Ordering::SeqCst), 0);
}

#[test]
fn start_interval_and_cycle_flag_pass_through() {
    let (rx, _) = exits();
    let probe = new_probe_settings(Duration::ZERO, 30, (rx, true));
    assert_eq!(probe.interval_secs.load(Ordering::SeqCst), 30);
    assert!(probe.full_cycle);
}

#[test]
fn returned_counters_are_the_ones_in_context() {
    let subkeys = NetworkKey::generate().unwrap().subkeys();
    let (_, _, ok_probe_count, interval, ctx) = build_member_context(
        AccessView::open(),
        crate::transport::net::id::NodeSecret::from_bytes(&[1; 32]).peer_id(),
        &subkeys,
        "n".into(),
        Duration::ZERO,
        5,
        exits(),
    );
    assert!(Arc::ptr_eq(&ok_probe_count, &ctx.probe.ok_probe_count));
    assert!(Arc::ptr_eq(&interval, &ctx.probe.interval_secs));
}
