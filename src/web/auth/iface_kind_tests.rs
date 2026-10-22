use std::net::Ipv4Addr;

use super::*;
use crate::web::auth::iface_os::view_from;

fn ip(s: &str) -> Ipv4Addr {
    s.parse().unwrap()
}

fn row(name: &str, addr: &str, mask: &str) -> (String, Ipv4Addr, Ipv4Addr, bool) {
    (name.to_string(), ip(addr), ip(mask), true)
}

const M24: &str = "255.255.255.0";

fn wan() -> (String, Ipv4Addr, Ipv4Addr, bool) {
    row("eth0", "192.168.1.100", M24)
}

fn router_with(second: (String, Ipv4Addr, Ipv4Addr, bool)) -> bool {
    is_router(&view_from(vec![wan(), second], Some(ip("192.168.1.100")), true))
}

#[test]
fn kind_of_tunnels() {
    for n in ["utun3", "wg0", "tailscale0", "tun0"] {
        assert_eq!(kind_of(n), Some(IfaceKind::Tunnel), "{n}");
    }
}

#[test]
fn kind_of_virtual() {
    for n in ["docker0", "Docker0", "br-0123456789ab", "veth1a2b", "virbr0", "vnic0", "bridge100"] {
        assert_eq!(kind_of(n), Some(IfaceKind::Virtual), "{n}");
    }
}

#[test]
fn kind_of_ordinary() {
    for n in ["br-lan", "br-guest", "br-0123", "eth0", "en0", "wlan0", "phy0-sta0", "pppoe-wan"] {
        assert_eq!(kind_of(n), None, "{n}");
    }
}

#[test]
fn router_needs_forwarding_and_route() {
    let lan = row("br-lan", "192.168.88.1", M24);
    let off = view_from(vec![wan(), lan.clone()], Some(ip("192.168.1.100")), false);
    assert!(!is_router(&off));
    let no_route = view_from(vec![wan(), lan.clone()], None, true);
    assert!(!is_router(&no_route));
    let tun = row("utun3", "192.168.1.100", M24);
    let virt_default = view_from(vec![tun, lan.clone()], Some(ip("192.168.1.100")), true);
    assert!(!is_router(&virt_default));
    assert!(router_with(lan));
}

#[test]
fn router_rejects_bad_second_interfaces() {
    for n in ["docker0", "br-0123456789ab", "bridge100", "vnic0", "wg0"] {
        assert!(!router_with(row(n, "192.168.88.1", M24)), "{n}");
    }
    assert!(!router_with(row("wlan0", "192.168.1.50", M24)));
    assert!(!router_with(row("en5", "169.254.3.4", "255.255.0.0")));
    let no_bc = ("br-lan".to_string(), ip("192.168.88.1"), ip(M24), false);
    assert!(!router_with(no_bc));
}

#[test]
fn travel_router_with_guest() {
    let v = view_from(
        vec![wan(), row("br-lan", "192.168.88.1", M24), row("br-guest", "192.168.9.1", M24)],
        Some(ip("192.168.1.100")),
        true,
    );
    assert!(is_router(&v));
}
