use super::*;
use crate::web::auth::iface_gate::Why;
use crate::web::auth::iface_os::view_from;
use std::net::Ipv4Addr;

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

#[test]
fn localhost_admits_only_loopback() {
    assert!(is_proxy_client_allowed(ListenAccess::Localhost, ip("127.0.0.1")));
    assert!(!is_proxy_client_allowed(ListenAccess::Localhost, ip("192.168.1.5")));
}

#[test]
fn localnet_admits_private_ranges_and_loopback() {
    for s in ["127.0.0.1", "10.1.2.3", "172.16.0.9", "192.168.1.5", "169.254.3.4"] {
        assert!(is_proxy_client_allowed(ListenAccess::Localnet, ip(s)), "{s}");
    }
}

#[test]
fn localnet_refuses_public_and_cgnat() {
    for s in ["8.8.8.8", "100.64.0.1", "172.32.0.1", "::1".replace("::1", "2001:db8::1").as_str()] {
        assert!(!is_proxy_client_allowed(ListenAccess::Localnet, ip(s)), "{s}");
    }
}

#[test]
fn deny_log_reports_first_per_period_only() {
    let mut log = DenyLog::default();
    let t = Instant::now();
    assert!(log.remember_if_first_in_period(ip("8.8.8.8"), t));
    assert!(!log.remember_if_first_in_period(ip("8.8.8.8"), t + Duration::from_secs(10)));
    assert!(log.remember_if_first_in_period(ip("8.8.4.4"), t));
    assert!(log.remember_if_first_in_period(ip("8.8.8.8"), t + DENY_LOG_PERIOD + Duration::from_secs(1)));
}

#[test]
fn deny_log_is_bounded() {
    let mut log = DenyLog::default();
    let t = Instant::now();
    for n in 0..DENY_LOG_CAP as u32 {
        assert!(log.remember_if_first_in_period(IpAddr::from(std::net::Ipv4Addr::from(0x0800_0000 + n)), t));
    }
    assert!(!log.remember_if_first_in_period(ip("9.9.9.9"), t));
}

fn v4(s: &str) -> Ipv4Addr {
    s.parse().unwrap()
}

fn sa(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

/// The OpenWrt travel-router view: WAN `eth0` (default route), `br-lan`, `br-guest`, forwarding enabled.
fn travel_router() -> IfaceView {
    let m24 = v4("255.255.255.0");
    view_from(
        vec![
            ("eth0".into(), v4("192.168.1.100"), m24, true),
            ("br-lan".into(), v4("192.168.88.1"), m24, true),
            ("br-guest".into(), v4("192.168.9.1"), m24, true),
        ],
        Some(v4("192.168.1.100")),
        true,
    )
}

fn adm(ifaces: IfaceFilter) -> Admission {
    Admission { access: ListenAccess::Localnet, ifaces }
}

#[test]
fn admit_conn_auto_closes_wan_and_opens_lan_and_guest() {
    let a = adm(IfaceFilter::Auto);
    let wan = admit_conn(&a, &travel_router(), sa("192.168.1.7:5000"), Some(sa("192.168.1.100:1080")));
    assert_eq!(wan.why, Why::AutoWan);
    let lan = admit_conn(&a, &travel_router(), sa("192.168.88.5:5000"), Some(sa("192.168.88.1:1080")));
    assert!(lan.allows());
    let guest = admit_conn(&a, &travel_router(), sa("192.168.9.5:5000"), Some(sa("192.168.9.1:1080")));
    assert!(guest.allows());
}

#[test]
fn admit_conn_without_local_addr_and_only_list_is_not_listed() {
    let a = adm(IfaceFilter::Only(vec!["br-lan".to_string()]));
    let v = admit_conn(&a, &travel_router(), sa("10.9.9.9:5000"), None);
    assert_eq!(v.why, Why::NotListed);
}

#[test]
fn iface_deny_text_names_iface_reason_and_command() {
    let a = adm(IfaceFilter::Auto);
    let client_addr = sa("192.168.1.7:5000");
    let v = admit_conn(&a, &travel_router(), client_addr, Some(sa("192.168.1.100:1080")));
    let text = iface_deny_text(client_addr, &v, &a);
    assert!(text.contains("192.168.1.7"), "{text}");
    assert!(text.contains("eth0"), "{text}");
    assert!(text.contains(v.why.meta().2), "{text}");
    assert!(text.contains("meshtown ifaces"), "{text}");
}

#[test]
fn report_iface_shares_period_with_report() {
    let mut log = DenyLog::default();
    let client_addr = sa("8.8.8.8:1");
    log.log_iface_deny(client_addr, "x");
    assert!(!log.remember_if_first_in_period(client_addr.ip(), Instant::now()), "address already recorded by log_iface_deny");
    let mut log = DenyLog::default();
    log.log_address_deny(client_addr);
    assert!(!log.remember_if_first_in_period(client_addr.ip(), Instant::now()));
}
