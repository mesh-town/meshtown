//! Tests for `access.rs`: peer admission by mode, binding, address on the local network.

use super::*;

fn v4(s: &str) -> IpAddr {
    IpAddr::V4(s.parse().unwrap())
}

fn v6(s: &str) -> IpAddr {
    IpAddr::V6(s.parse().unwrap())
}

mod localhost {
    use super::*;

    #[test]
    fn loopback_range_allowed() {
        assert!(is_client_allowed(ListenAccess::Localhost, v4("127.0.0.1")));
        assert!(is_client_allowed(ListenAccess::Localhost, v4("127.5.5.5")));
    }

    #[test]
    fn private_refused() {
        assert!(!is_client_allowed(
            ListenAccess::Localhost,
            v4("192.168.1.5")
        ));
        assert!(!is_client_allowed(ListenAccess::Localhost, v4("10.0.0.1")));
    }
}

mod localnet {
    use super::*;

    #[test]
    fn private_and_link_local_allowed() {
        for s in [
            "10.0.0.1",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.1",
            "169.254.1.1",
        ] {
            assert!(is_client_allowed(ListenAccess::Localnet, v4(s)), "{s}");
        }
    }

    #[test]
    fn edges_refused() {
        for s in [
            "172.32.0.1",
            "100.64.0.1",
            "8.8.8.8",
            "0.0.0.0",
            "255.255.255.255",
        ] {
            assert!(!is_client_allowed(ListenAccess::Localnet, v4(s)), "{s}");
        }
    }

    #[test]
    fn mapped_v6_judged_as_v4() {
        assert!(is_client_allowed(
            ListenAccess::Localnet,
            v6("::ffff:192.168.0.1")
        ));
        assert!(!is_client_allowed(
            ListenAccess::Localnet,
            v6("::ffff:8.8.8.8")
        ));
        assert!(is_client_allowed(
            ListenAccess::Localhost,
            v6("::ffff:127.0.0.1")
        ));
    }

    #[test]
    fn plain_v6_refused() {
        assert!(!is_client_allowed(ListenAccess::Localnet, v6("::1")));
        assert!(!is_client_allowed(ListenAccess::Localnet, v6("fe80::1")));
    }
}

mod bind {
    use super::*;

    #[test]
    fn localhost_is_loopback() {
        assert_eq!(bind_ip(ListenAccess::Localhost), Ipv4Addr::LOCALHOST);
    }

    #[test]
    fn localnet_is_unspecified() {
        assert_eq!(bind_ip(ListenAccess::Localnet), Ipv4Addr::UNSPECIFIED);
    }
}

mod lan_from {
    use super::*;

    #[test]
    fn private_kept() {
        let got = lan_from(Some(v4("192.168.1.7")));
        assert_eq!(got, Some(Ipv4Addr::new(192, 168, 1, 7)));
    }

    #[test]
    fn public_loopback_cgnat_v6_none_dropped() {
        assert_eq!(lan_from(Some(v4("8.8.8.8"))), None);
        assert_eq!(lan_from(Some(v4("127.0.0.1"))), None);
        assert_eq!(lan_from(Some(v4("100.64.0.1"))), None);
        assert_eq!(lan_from(Some(v6("fe80::1"))), None);
        assert_eq!(lan_from(None), None);
    }
}

mod lan_ipv4 {
    use super::*;

    #[test]
    fn result_is_none_or_private() {
        match lan_ipv4() {
            None => {}
            Some(ip) => assert!(is_client_allowed(ListenAccess::Localnet, IpAddr::V4(ip))),
        }
    }
}

#[test]
fn lan_pick_filters_and_orders_default_first() {
    let ip = |a, b, c, d| Ipv4Addr::new(a, b, c, d);
    let all = vec![
        ("wan".to_string(), ip(8, 8, 8, 8)),
        ("lo".to_string(), ip(127, 0, 0, 1)),
        ("wg0".to_string(), ip(100, 64, 0, 2)),
        ("eth1".to_string(), ip(10, 0, 0, 5)),
        ("br-lan".to_string(), ip(192, 168, 8, 1)),
        ("br-guest".to_string(), ip(172, 16, 5, 1)),
        ("br-lan".to_string(), ip(192, 168, 8, 1)),
    ];
    let names = |v: Vec<IfaceAddr>| v.into_iter().map(|(n, _)| n).collect::<Vec<_>>();
    assert_eq!(
        names(lan_addrs_pick(all.clone(), None)),
        ["br-guest", "br-lan", "eth1"]
    );
    assert_eq!(
        names(lan_addrs_pick(all.clone(), Some(ip(10, 0, 0, 5)))),
        ["eth1", "br-guest", "br-lan"]
    );
    assert_eq!(
        names(lan_addrs_pick(all, Some(ip(8, 8, 8, 8)))),
        ["br-guest", "br-lan", "eth1"],
        "a public default is not included"
    );
    assert!(lan_addrs_pick(vec![], None).is_empty());
}
