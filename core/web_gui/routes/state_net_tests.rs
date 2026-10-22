use std::net::Ipv4Addr;

use serde_json::json;

use super::*;
use crate::web::auth::iface_os::view_from;

fn ip(s: &str) -> Ipv4Addr {
    s.parse().unwrap()
}

fn travel_router() -> IfaceView {
    let row = |n: &str, a: &str| (n.to_string(), ip(a), ip("255.255.255.0"), true);
    let rows = vec![row("br-lan", "192.168.88.1"), row("eth0", "192.168.1.100"), row("br-guest", "192.168.9.1")];
    view_from(rows, Some(ip("192.168.1.100")), true)
}

fn lan(items: &[(&str, &str)]) -> Vec<SocksLan> {
    items.iter().map(|(i, a)| SocksLan { addr: a.to_string(), iface: i.to_string() }).collect()
}

const SNAP: Option<&str> = Some("http://127.0.0.1:1080/proxy.pac");

#[test]
fn pac_for_keeps_snapshot_when_nothing_to_choose() {
    let l = lan(&[("br-lan", "192.168.88.1:1080")]);
    assert_eq!(pac_for(SNAP, "localhost", Some("br-lan"), &l).as_deref(), SNAP);
    assert_eq!(pac_for(SNAP, "localnet", None, &l).as_deref(), SNAP);
    assert_eq!(pac_for(SNAP, "localnet", Some("br-lan"), &[]).as_deref(), SNAP);
}

#[test]
fn pac_for_picks_the_address_in_the_callers_network() {
    let l = lan(&[("br-guest", "192.168.9.1:1080"), ("br-lan", "192.168.88.1:1080")]);
    let want = Some("http://192.168.88.1:1080/proxy.pac".to_string());
    assert_eq!(pac_for(SNAP, "localnet", Some("br-lan"), &l), want);
    let only_lan = lan(&[("br-lan", "192.168.88.1:1080")]);
    assert_eq!(pac_for(SNAP, "localnet", Some("eth0"), &only_lan), want);
}

fn snap(enabled: bool, access: &str, ifaces: Option<Vec<String>>) -> NodeSnapshot {
    let mut s = NodeSnapshot::default();
    s.socks.enabled = enabled;
    s.socks.access = access.to_string();
    s.socks.ifaces = ifaces;
    s
}

fn auth(mode: ListenAccess) -> AuthState {
    AuthState::new("mesh_test".to_string(), None, mode)
}

fn conn(peer: &str, local: &str) -> ConnInfo {
    ConnInfo { client_addr: peer.parse().unwrap(), local: local.parse().unwrap() }
}

fn run(s: &NodeSnapshot, a: &AuthState, c: Option<&ConnInfo>) -> NetState {
    net_state(s, a, &travel_router(), c, vec![])
}

#[test]
fn side_is_socks_when_proxy_serves_the_network() {
    let n = run(&snap(true, "localnet", Some(vec!["br-lan".into()])), &auth(ListenAccess::Localhost), None);
    assert_eq!(n.side, "socks");
    let open: Vec<_> = n.networks.iter().filter(|e| e.open).map(|e| e.name.as_str()).collect();
    assert_eq!(open, ["br-lan"]);
}

#[test]
fn side_is_web_when_only_the_page_listens() {
    let n = run(&snap(false, "localnet", None), &auth(ListenAccess::Localnet), None);
    assert_eq!(n.side, "web");
    assert!(n.networks.iter().any(|e| e.open));
}

#[test]
fn side_is_none_when_both_are_localhost() {
    let n = run(&snap(true, "localhost", None), &auth(ListenAccess::Localhost), None);
    assert_eq!(n.side, "none");
    assert!(n.networks.iter().all(|e| !e.open));
}

#[test]
fn via_iface_names_the_network_only_for_remote_callers() {
    let s = snap(true, "localnet", None);
    let a = auth(ListenAccess::Localnet);
    let loopback = conn("127.0.0.1:5000", "127.0.0.1:7070");
    assert_eq!(run(&s, &a, Some(&loopback)).via_iface, None);
    let remote = conn("192.168.88.20:5000", "192.168.88.1:7070");
    assert_eq!(run(&s, &a, Some(&remote)).via_iface.as_deref(), Some("br-lan"));
    let own = conn("192.168.88.1:5000", "192.168.88.1:7070");
    assert_eq!(run(&s, &a, Some(&own)).via_iface, None);
    assert_eq!(run(&s, &a, None).via_iface, None);
}

#[test]
fn apply_net_adds_keys_and_replaces_pac() {
    let mut s = snap(true, "localnet", Some(vec!["br-lan".into()]));
    s.socks.lan = lan(&[("br-lan", "192.168.88.1:1080")]);
    s.socks.pac_url = SNAP.map(str::to_string);
    let remote = conn("192.168.88.20:5000", "192.168.88.1:7070");
    let net = net_state(&s, &auth(ListenAccess::Localnet), &travel_router(), Some(&remote), vec!["socks.ifaces"]);
    let mut v = json!({"node": {}, "socks": {"pac_url": null}});
    apply_net(&mut v, net);
    for k in ["router", "via_iface", "networks", "networks_side"] {
        assert!(v["node"].get(k).is_some(), "{k}");
    }
    assert_eq!(v["restart_pending"], json!(["socks.ifaces"]));
    assert_eq!(v["web"]["listening"], "localnet");
    assert_eq!(v["web"]["ifaces"], json!(null));
    assert_eq!(v["socks"]["pac_url"], "http://192.168.88.1:1080/proxy.pac");
    assert!(v["node"]["networks"][0].get("kind").is_none());
}

#[test]
fn apply_net_does_not_invent_socks() {
    let net = run(&snap(false, "localhost", None), &auth(ListenAccess::Localhost), None);
    let mut v = json!({"node": {}});
    apply_net(&mut v, net);
    assert!(v.get("socks").is_none());
}
