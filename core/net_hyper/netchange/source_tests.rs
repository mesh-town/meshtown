//! Network fingerprint and transitions: pure functions, no OS.

use super::*;
use std::net::{Ipv4Addr, Ipv6Addr};

fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(a, b, c, d))
}

fn fp(addrs: Vec<IpAddr>, route: Option<&str>, woke: Option<Instant>) -> Option<Fingerprint> {
    fingerprint(addrs, route, woke)
}

#[test]
fn fingerprint_is_none_with_only_loopback_and_link_local() {
    let addrs = vec![v4(127, 0, 0, 1), v4(169, 254, 3, 4)];
    assert_eq!(fp(addrs, Some("en0"), None), None);
}

#[test]
fn fingerprint_is_none_without_default_route() {
    assert_eq!(fp(vec![v4(192, 168, 1, 5)], None, None), None);
}

#[test]
fn fingerprint_ignores_address_order() {
    let a = fp(vec![v4(10, 0, 0, 1), v4(192, 168, 1, 5)], Some("en0"), None);
    let b = fp(vec![v4(192, 168, 1, 5), v4(10, 0, 0, 1)], Some("en0"), None);
    assert!(a.is_some());
    assert_eq!(a, b);
}

#[test]
fn fingerprint_differs_on_new_address() {
    let a = fp(vec![v4(192, 168, 1, 5)], Some("en0"), None);
    let b = fp(vec![v4(192, 168, 1, 6)], Some("en0"), None);
    assert_ne!(a, b);
}

#[test]
fn fingerprint_differs_after_wake() {
    let a = fp(vec![v4(192, 168, 1, 5)], Some("en0"), None);
    let b = fp(vec![v4(192, 168, 1, 5)], Some("en0"), Some(Instant::now()));
    assert_ne!(a, b);
}

#[test]
fn fingerprint_ignores_ipv6() {
    let v6 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
    let a = fp(vec![v4(192, 168, 1, 5)], Some("en0"), None);
    let b = fp(vec![v4(192, 168, 1, 5), v6], Some("en0"), None);
    assert!(a.is_some());
    assert_eq!(a, b);
}

#[test]
fn step_maps_transitions() {
    let a = fp(vec![v4(10, 0, 0, 1)], Some("en0"), None);
    let b = fp(vec![v4(10, 0, 0, 2)], Some("en0"), None);
    let row = [a.clone(), a, b.clone(), None, None, b];
    let got: Vec<NetEvent> = row.windows(2).filter_map(|w| net_event_between(&w[0], &w[1])).collect();
    assert_eq!(got, vec![NetEvent::Changed, NetEvent::Lost, NetEvent::Changed]);
}

#[test]
fn step_from_none_to_none_is_silent() {
    assert_eq!(net_event_between(&None, &None), None);
}
