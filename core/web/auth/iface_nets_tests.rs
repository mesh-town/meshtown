use std::net::Ipv4Addr;

use super::*;
use crate::web::auth::iface_os::view_from;

fn ip(s: &str) -> Ipv4Addr {
    s.parse().unwrap()
}

fn row(name: &str, addr: &str) -> (String, Ipv4Addr, Ipv4Addr, bool) {
    (name.to_string(), ip(addr), ip("255.255.255.0"), true)
}

fn travel_router_rows() -> Vec<(String, Ipv4Addr, Ipv4Addr, bool)> {
    vec![
        row("br-lan", "192.168.88.1"),
        row("eth0", "192.168.1.100"),
        row("br-guest", "192.168.9.1"),
    ]
}

fn travel_router() -> IfaceView {
    view_from(travel_router_rows(), Some(ip("192.168.1.100")), true)
}

fn only(names: &[&str]) -> IfaceFilter {
    IfaceFilter::Only(names.iter().map(|s| s.to_string()).collect())
}

const NET: ListenAccess = ListenAccess::Localnet;

fn flags(list: &[NetEntry]) -> Vec<(&str, bool, By)> {
    list.iter().map(|e| (e.name.as_str(), e.open, e.by)).collect()
}

#[test]
fn auto_closes_only_provider_network() {
    let list = networks(NET, &IfaceFilter::Auto, &travel_router());
    let want = vec![
        ("eth0", false, By::Auto),
        ("br-guest", true, By::Auto),
        ("br-lan", true, By::Auto),
    ];
    assert_eq!(flags(&list), want);
    assert!(list[0].default_route);
}

#[test]
fn only_lists() {
    let list = networks(NET, &only(&["br-lan"]), &travel_router());
    let want = vec![
        ("eth0", false, By::List),
        ("br-guest", false, By::List),
        ("br-lan", true, By::List),
    ];
    assert_eq!(flags(&list), want);
    let both = networks(NET, &only(&["br-lan", "eth0"]), &travel_router());
    assert_eq!(flags(&both)[0], ("eth0", true, By::List));
}

#[test]
fn missing_name_is_absent_entry() {
    let list = networks(NET, &only(&["br-lan", "wlan9"]), &travel_router());
    let last = list.last().unwrap();
    assert_eq!(last.name, "wlan9");
    assert!(!last.present && !last.open && last.addrs.is_empty());
    assert_eq!(last.by, By::List);
}

#[test]
fn any_and_localhost() {
    let any = networks(NET, &IfaceFilter::Any, &travel_router());
    assert!(any.iter().all(|e| e.open && e.by == By::List));
    let none = networks(ListenAccess::Localhost, &IfaceFilter::Auto, &travel_router());
    assert!(none.iter().all(|e| !e.open && e.by == By::None));
}

#[test]
fn kind_key_in_json() {
    let mut rows = travel_router_rows();
    rows.push(row("docker0", "172.17.0.1"));
    let v = view_from(rows, Some(ip("192.168.1.100")), true);
    let list = networks(NET, &IfaceFilter::Auto, &v);
    let find = |n: &str| list.iter().find(|e| e.name == n).unwrap();
    assert_eq!(find("docker0").kind, Some(IfaceKind::Virtual));
    let docker = serde_json::to_value(find("docker0")).unwrap();
    assert_eq!(docker["kind"], "virtual");
    let eth = serde_json::to_value(find("eth0")).unwrap();
    assert!(eth.get("kind").is_none());
}

fn lan() -> Vec<IfaceAddr> {
    vec![
        ("eth0".to_string(), ip("192.168.1.100")),
        ("br-guest".to_string(), ip("192.168.9.1")),
        ("br-lan".to_string(), ip("192.168.88.1")),
    ]
}

fn names(v: Vec<IfaceAddr>) -> Vec<String> {
    v.into_iter().map(|(n, _)| n).collect()
}

#[test]
fn admitted_lan_cases() {
    let auto = admitted_lan(NET, &IfaceFilter::Auto, &travel_router(), lan());
    assert_eq!(names(auto), ["br-guest", "br-lan"]);
    let one = admitted_lan(NET, &only(&["br-lan"]), &travel_router(), lan());
    assert_eq!(names(one), ["br-lan"]);
    assert!(admitted_lan(ListenAccess::Localhost, &IfaceFilter::Auto, &travel_router(), lan()).is_empty());
    let plain = view_from(travel_router_rows(), Some(ip("192.168.1.100")), false);
    assert_eq!(admitted_lan(NET, &IfaceFilter::Auto, &plain, lan()).len(), 3);
}
