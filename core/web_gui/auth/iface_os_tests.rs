use std::net::Ipv4Addr;
use std::sync::Arc;

use super::*;

#[test]
fn forwarding_text() {
    assert!(forwarding_from_text("1\n"));
    for s in ["0\n", "", "x"] {
        assert!(!forwarding_from_text(s), "{s:?}");
    }
}

#[test]
fn view_from_computes_prefix_and_keeps_broadcast() {
    let ip = Ipv4Addr::new(192, 168, 1, 5);
    let mask = Ipv4Addr::new(255, 255, 255, 0);
    let v = view_from(
        vec![("a".into(), ip, mask, true), ("b".into(), ip, mask, false)],
        Some(ip),
        true,
    );
    assert_eq!(v.addrs[0].prefix, 24);
    assert!(v.addrs[0].broadcast);
    assert!(!v.addrs[1].broadcast);
    assert_eq!(v.default_ip, Some(ip));
    assert!(v.forwarding);
}

#[test]
fn current_view_is_cached() {
    let a = current_view();
    let b = current_view();
    assert!(Arc::ptr_eq(&a, &b));
    let _ = forwarding();
}
