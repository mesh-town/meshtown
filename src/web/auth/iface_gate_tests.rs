use std::net::{IpAddr, Ipv4Addr};

use super::*;
use crate::web::auth::iface_os::view_from;

fn ip(s: &str) -> Ipv4Addr {
    s.parse().unwrap()
}

fn row(name: &str, addr: &str) -> (String, Ipv4Addr, Ipv4Addr, bool) {
    (name.to_string(), ip(addr), ip("255.255.255.0"), true)
}

fn travel_router_with(forwarding: bool) -> IfaceView {
    let rows = vec![
        row("eth0", "192.168.1.100"),
        row("br-lan", "192.168.88.1"),
        row("br-guest", "192.168.9.1"),
    ];
    view_from(rows, Some(ip("192.168.1.100")), forwarding)
}

fn travel_router() -> IfaceView {
    travel_router_with(true)
}

fn only(names: &[&str]) -> IfaceFilter {
    IfaceFilter::Only(names.iter().map(|s| s.to_string()).collect())
}

fn j(mode: ListenAccess, f: &IfaceFilter, v: &IfaceView, peer: &str, local: &str) -> Why {
    let l: Option<IpAddr> = Some(local.parse().unwrap());
    judge(mode, f, v, peer.parse().unwrap(), l).why
}

const NET: ListenAccess = ListenAccess::Localnet;

#[test]
fn rows_1_to_3() {
    let v = travel_router();
    for f in [IfaceFilter::Auto, IfaceFilter::Any, only(&["x"])] {
        assert_eq!(j(NET, &f, &v, "127.0.0.1", "127.0.0.1"), Why::Loopback);
        assert_eq!(j(ListenAccess::Localhost, &f, &v, "127.0.0.1", "127.0.0.1"), Why::Loopback);
        assert_eq!(j(ListenAccess::Localhost, &f, &v, "192.168.88.20", "192.168.88.1"), Why::Localhost);
        assert_eq!(j(NET, &f, &v, "192.168.88.1", "192.168.88.1"), Why::SelfHost);
    }
}

#[test]
fn rows_4_to_9() {
    let v = travel_router();
    assert_eq!(j(NET, &IfaceFilter::Any, &v, "192.168.1.20", "192.168.1.100"), Why::Any);
    let lan = only(&["br-lan"]);
    assert_eq!(j(NET, &lan, &v, "192.168.88.20", "192.168.88.1"), Why::Listed);
    assert_eq!(j(NET, &lan, &v, "192.168.1.20", "192.168.1.100"), Why::NotListed);
    assert_eq!(j(NET, &lan, &v, "10.9.9.9", "10.1.1.1"), Why::NotListed);
    let auto = IfaceFilter::Auto;
    assert_eq!(j(NET, &auto, &v, "192.168.1.20", "192.168.1.100"), Why::AutoWan);
    assert_eq!(j(NET, &auto, &v, "192.168.88.20", "192.168.88.1"), Why::Open);
    assert_eq!(j(NET, &auto, &v, "10.9.9.9", "10.1.1.1"), Why::Open);
    let plain = travel_router_with(false);
    assert_eq!(j(NET, &auto, &plain, "192.168.1.20", "192.168.1.100"), Why::Open);
}

#[test]
fn rows_10_and_11_ingress_by_client_subnet() {
    let v = travel_router();
    let lan = only(&["br-lan"]);
    assert_eq!(j(NET, &lan, &v, "172.16.0.5", "192.168.88.1"), Why::Listed);
    assert_eq!(j(NET, &lan, &v, "192.168.1.20", "192.168.88.1"), Why::NotListed);
}

#[test]
fn rows_12_to_14_and_answers_395_396() {
    let v = travel_router();
    let both = only(&["br-lan", "eth0"]);
    assert_eq!(j(NET, &both, &v, "192.168.1.20", "192.168.1.100"), Why::Listed);
    assert_eq!(j(NET, &IfaceFilter::Auto, &v, "192.168.9.20", "192.168.9.1"), Why::Open);
    assert_eq!(j(NET, &only(&["br-lan"]), &v, "192.168.9.20", "192.168.9.1"), Why::NotListed);
}

#[test]
fn mapped_loopback_ipv6_and_localhost_self() {
    let v = travel_router();
    let f = IfaceFilter::Auto;
    let mapped: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
    assert_eq!(judge(NET, &f, &v, mapped, None).why, Why::Loopback);
    let v6: IpAddr = "fd00::5".parse().unwrap();
    let verdict = judge(NET, &f, &v, v6, None);
    assert_eq!(verdict.why, Why::NotListed);
    assert!(!verdict.allows());
    let own: IpAddr = "192.168.88.1".parse().unwrap();
    let r = judge(ListenAccess::Localhost, &f, &v, own, None);
    assert_eq!(r.why, Why::Localhost);
    assert!(!r.allows());
}

#[test]
fn ingress_cases() {
    let v = travel_router();
    assert_eq!(ingress(&v, ip("192.168.9.20"), None), Some("br-guest"));
    assert_eq!(ingress(&v, ip("172.16.0.5"), Some(ip("192.168.88.1"))), Some("br-lan"));
    assert_eq!(ingress(&v, ip("172.16.0.5"), Some(ip("203.0.113.9"))), None);
    let two = view_from(
        vec![row("a0", "10.0.0.1"), row("b0", "10.0.0.2")],
        None,
        false,
    );
    assert_eq!(ingress(&two, ip("10.0.0.7"), Some(ip("10.0.0.2"))), Some("b0"));
    assert_eq!(ingress(&two, ip("10.0.0.7"), None), Some("a0"));
}

#[test]
fn why_meta() {
    for w in [Why::Listed, Why::Any, Why::Open, Why::Loopback, Why::SelfHost] {
        assert!(w.meta().0, "{w:?}");
    }
    for w in [Why::NotListed, Why::AutoWan, Why::Localhost] {
        assert!(!w.meta().0, "{w:?}");
    }
    for w in [Why::Listed, Why::NotListed, Why::Any] {
        assert_eq!(w.meta().1, By::List);
    }
    assert_eq!(Why::AutoWan.meta().1, By::Auto);
    assert_eq!(Why::Open.meta().1, By::Auto);
    assert_eq!(Why::Localhost.meta().1, By::None);
}
