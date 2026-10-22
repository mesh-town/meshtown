//! Tests for the order of direct addresses (`addr_order.rs`).

use std::net::{IpAddr, SocketAddr};

use super::*;

mod order_direct {
    use super::*;

    mod with_v4_route_and_vm_bridges {
        use super::*;

        #[test]
        fn lan_address_goes_first_bridges_keep_order() {
            // given
            let mut a = addrs(&[
                "10.99.129.2:54674",
                "10.20.30.2:54674",
                "192.168.88.129:54674",
            ]);
            // when
            order_direct(&mut a, &route(Some("192.168.88.129"), None));
            // then
            let want = addrs(&[
                "192.168.88.129:54674",
                "10.99.129.2:54674",
                "10.20.30.2:54674",
            ]);
            assert_eq!(a, want);
        }

        #[test]
        fn lan_address_survives_truncate_to_two() {
            // given
            let mut a = addrs(&[
                "10.99.129.2:54674",
                "10.20.30.2:54674",
                "192.168.88.129:54674",
            ]);
            // when
            order_direct(&mut a, &route(Some("192.168.88.129"), None));
            a.truncate(2);
            // then
            assert!(a.contains(&sa("192.168.88.129:54674")), "{a:?}");
        }
    }

    mod with_v4_and_v6_routes {
        use super::*;

        #[test]
        fn both_route_addresses_first_v4_before_v6() {
            // given
            let mut a = addrs(&[
                "10.99.129.2:1",
                "192.168.1.5:1",
                "[2a01:4f8::5]:1",
                "[fd00::1]:1",
            ]);
            // when
            order_direct(&mut a, &route(Some("192.168.1.5"), Some("2a01:4f8::5")));
            // then
            let want = addrs(&[
                "192.168.1.5:1",
                "[2a01:4f8::5]:1",
                "10.99.129.2:1",
                "[fd00::1]:1",
            ]);
            assert_eq!(a, want);
        }
    }

    mod without_route {
        use super::*;

        #[test]
        fn private_only_keeps_order() {
            // given
            let orig = addrs(&["10.0.0.1:1", "10.0.0.2:1", "192.168.0.3:1", "[fd00::1]:1"]);
            let mut a = orig.clone();
            // when
            order_direct(&mut a, &PrimaryIps::NONE);
            // then
            assert_eq!(a, orig);
        }

        #[test]
        fn public_address_overtakes_bridges() {
            // given
            let mut a = addrs(&[
                "10.99.129.2:1",
                "10.20.30.2:1",
                "172.17.0.1:1",
                "198.51.100.7:1",
            ]);
            // when
            order_direct(&mut a, &PrimaryIps::NONE);
            // then
            let want = addrs(&[
                "198.51.100.7:1",
                "10.99.129.2:1",
                "10.20.30.2:1",
                "172.17.0.1:1",
            ]);
            assert_eq!(a, want);
        }
    }

    mod route_address_not_in_list {
        use super::*;

        #[test]
        fn order_is_as_without_route() {
            // given
            let orig = addrs(&["10.0.0.1:1", "10.0.0.2:1", "192.168.0.3:1"]);
            let mut a = orig.clone();
            // when
            order_direct(&mut a, &route(Some("192.168.9.9"), None));
            // then
            assert_eq!(a, orig);
        }
    }

    mod empty_list {
        use super::*;

        #[test]
        fn does_not_panic() {
            // given
            let mut a: Vec<SocketAddr> = vec![];
            // when
            order_direct(&mut a, &route(Some("192.168.9.9"), None));
            // then
            assert!(a.is_empty());
        }
    }

    mod with_foreign_tunnel_address {
        use super::*;

        #[test]
        fn tunnel_goes_after_lan_and_public() {
            // given
            let mut a = addrs(&[
                "100.64.0.7:1",
                "10.99.0.2:1",
                "192.168.1.2:1",
                "203.0.113.5:1",
            ]);
            // when
            order_direct(&mut a, &PrimaryIps::NONE);
            // then
            let want = addrs(&[
                "203.0.113.5:1",
                "10.99.0.2:1",
                "192.168.1.2:1",
                "100.64.0.7:1",
            ]);
            assert_eq!(a, want, "tunnel 100.64/10 goes last, public goes first");
        }

        #[test]
        fn tunnel_is_last_even_as_default_route() {
            // given
            let mut a = addrs(&["100.64.0.7:1", "10.99.0.2:1", "192.168.1.2:1"]);
            // when
            order_direct(&mut a, &route(Some("100.64.0.7"), None));
            // then
            let want = addrs(&["10.99.0.2:1", "192.168.1.2:1", "100.64.0.7:1"]);
            assert_eq!(
                a, want,
                "a Tailscale exit for all traffic does not put the tunnel first"
            );
        }

        #[test]
        fn lone_tunnel_is_kept() {
            // given
            let mut a = addrs(&["100.64.0.7:1"]);
            // when
            order_direct(&mut a, &PrimaryIps::NONE);
            // then
            assert_eq!(
                a,
                addrs(&["100.64.0.7:1"]),
                "the only address is not removed"
            );
        }
    }
}

mod is_foreign_tunnel {
    use super::*;

    #[test]
    fn cgnat_and_mapped_are_tunnel() {
        for s in ["100.64.0.0", "100.127.255.255", "::ffff:100.64.0.1"] {
            assert!(
                is_foreign_tunnel(ip(s)),
                "{s} is in 100.64/10, must be a tunnel"
            );
        }
    }

    #[test]
    fn boundaries_and_lan_are_not_tunnel() {
        for s in ["100.63.255.255", "100.128.0.0", "192.168.1.2"] {
            assert!(
                !is_foreign_tunnel(ip(s)),
                "{s} is outside 100.64/10, not a tunnel"
            );
        }
    }
}

mod is_public {
    use super::*;

    #[test]
    fn private_loopback_linklocal_cgnat_fakeip_unspecified_are_not() {
        let not_public = [
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "127.0.0.1",
            "169.254.1.1",
            "100.64.0.1",
            "198.18.0.1",
            "0.0.0.0",
            "::1",
            "fe80::1",
            "fd00::1",
            "::",
        ];
        for s in not_public {
            assert!(!is_public(ip(s)), "{s} must not be public");
        }
    }

    #[test]
    fn global_addresses_are_public() {
        for s in ["5.6.7.8", "8.8.8.8", "2a01:4f8::1"] {
            assert!(is_public(ip(s)), "{s} must be public");
        }
    }

    #[test]
    fn cgnat_boundaries_are_public() {
        for s in ["100.63.255.255", "100.128.0.0"] {
            assert!(is_public(ip(s)), "{s} is outside 100.64/10, must be public");
        }
    }
}

mod route_source {
    use super::*;

    #[test]
    fn on_loopback_returns_source_address() {
        // given / when
        let got = route_source(sa("127.0.0.1:0"), sa("127.0.0.1:9"));
        // then
        assert_eq!(got, Some(ip("127.0.0.1")));
    }

    #[test]
    fn mismatched_families_give_none() {
        // given / when
        let got = route_source(sa("127.0.0.1:0"), sa("[::1]:9"));
        // then
        assert_eq!(got, None);
    }
}

mod truncate_ranked {
    use super::*;

    #[test]
    fn lan_bridges_and_tunnel_keep_tunnel_last() {
        // given
        let mut a = addrs(&[
            "192.168.1.2:1",
            "10.99.0.2:1",
            "10.20.30.2:1",
            "100.64.0.7:1",
        ]);
        // when
        truncate_ranked(&mut a, 2, &PrimaryIps::NONE);
        // then
        assert_eq!(a, addrs(&["192.168.1.2:1", "100.64.0.7:1"]));
    }

    #[test]
    fn public_address_lets_tunnel_go() {
        // given
        let mut a = addrs(&[
            "203.0.113.5:1",
            "10.99.0.2:1",
            "10.20.30.2:1",
            "100.64.0.7:1",
        ]);
        // when
        truncate_ranked(&mut a, 2, &PrimaryIps::NONE);
        // then
        assert_eq!(a, addrs(&["203.0.113.5:1", "10.99.0.2:1"]));
    }

    #[test]
    fn route_address_lets_tunnel_go() {
        // given
        let mut a = addrs(&["192.168.1.2:1", "10.99.0.2:1", "100.64.0.7:1"]);
        // when
        truncate_ranked(&mut a, 2, &route(Some("192.168.1.2"), None));
        // then
        assert_eq!(a, addrs(&["192.168.1.2:1", "10.99.0.2:1"]));
    }

    #[test]
    fn best_tunnel_takes_the_slot() {
        // given
        let mut a = addrs(&[
            "10.99.0.2:1",
            "10.20.30.2:1",
            "100.64.0.7:1",
            "100.64.0.9:1",
        ]);
        // when
        truncate_ranked(&mut a, 2, &PrimaryIps::NONE);
        // then
        assert_eq!(a, addrs(&["10.99.0.2:1", "100.64.0.7:1"]));
    }

    #[test]
    fn short_list_is_untouched() {
        // given
        let mut a = addrs(&["10.99.0.2:1", "100.64.0.7:1"]);
        // when
        truncate_ranked(&mut a, 2, &PrimaryIps::NONE);
        // then
        assert_eq!(a, addrs(&["10.99.0.2:1", "100.64.0.7:1"]));
    }
}

mod probe {
    use super::*;

    #[test]
    fn does_not_panic_and_never_returns_unspecified() {
        // given / when
        let p = PrimaryIps::probe();
        // then
        if let Some(v4) = p.v4 {
            assert!(!v4.is_unspecified(), "v4 = {v4}");
        }
        if let Some(v6) = p.v6 {
            assert!(!v6.is_unspecified(), "v6 = {v6}");
        }
    }
}

fn sa(s: &str) -> SocketAddr {
    s.parse().unwrap()
}

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

fn addrs(list: &[&str]) -> Vec<SocketAddr> {
    list.iter().map(|s| sa(s)).collect()
}

fn route(v4: Option<&str>, v6: Option<&str>) -> PrimaryIps {
    PrimaryIps {
        v4: v4.map(|s| s.parse().unwrap()),
        v6: v6.map(|s| s.parse().unwrap()),
    }
}
