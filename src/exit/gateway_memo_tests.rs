use super::*;
use crate::net::facade::test_support::pid;

#[test]
fn refusal_is_remembered_until_the_ttl_passes() {
    let memo = GatewayMemo::default();
    let t0 = Instant::now();
    assert!(!memo.refused_recently(pid(1), t0));
    memo.remember_refused(pid(1), t0);
    assert!(memo.refused_recently(pid(1), t0 + GATEWAY_REFUSAL_TTL - Duration::from_secs(1)));
    assert!(!memo.refused_recently(pid(1), t0 + GATEWAY_REFUSAL_TTL));
}

#[test]
fn another_peer_is_not_affected() {
    let memo = GatewayMemo::default();
    let t0 = Instant::now();
    memo.remember_refused(pid(1), t0);
    assert!(!memo.refused_recently(pid(2), t0));
}

#[test]
fn remembering_again_extends_the_window() {
    let memo = GatewayMemo::default();
    let t0 = Instant::now();
    memo.remember_refused(pid(1), t0);
    let later = t0 + GATEWAY_REFUSAL_TTL - Duration::from_secs(1);
    memo.remember_refused(pid(1), later);
    assert!(memo.refused_recently(pid(1), t0 + GATEWAY_REFUSAL_TTL + Duration::from_secs(60)));
}

#[test]
fn clones_share_the_memory() {
    let memo = GatewayMemo::default();
    let t0 = Instant::now();
    memo.clone().remember_refused(pid(1), t0);
    assert!(memo.refused_recently(pid(1), t0));
}
