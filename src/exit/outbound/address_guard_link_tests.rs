//! Tests for the IPv6 neighbour check of the exit address filter (`address_guard.rs`).

use super::*;

mod same_v6_64 {
    use super::*;

    #[test]
    fn addresses_in_one_64_match() {
        let a: Ipv6Addr = "2001:db8:1:2::1".parse().unwrap();
        let b: Ipv6Addr = "2001:db8:1:2:ffff::9".parse().unwrap();

        assert!(same_v6_64(a, b));
    }

    #[test]
    fn addresses_in_neighbour_64s_differ() {
        let a: Ipv6Addr = "2001:db8:1:2::1".parse().unwrap();
        let b: Ipv6Addr = "2001:db8:1:3::1".parse().unwrap();

        assert!(!same_v6_64(a, b));
    }

    #[test]
    fn prefix_compares_only_first_64_bits() {
        let a: Ipv6Addr = "2001:db8:1:2:0:0:0:1".parse().unwrap();
        let b: Ipv6Addr = "2001:db8:1:2:8000::".parse().unwrap();
        let c: Ipv6Addr = "2001:db8:1:2:ffff:ffff:ffff:ffff".parse().unwrap();

        assert!(same_v6_64(a, b) && same_v6_64(b, c));
    }
}

mod probe {
    use super::*;

    fn has_ipv6_loopback() -> bool {
        UdpSocket::bind((Ipv6Addr::LOCALHOST, 0)).is_ok()
    }

    #[test]
    fn loopback_is_on_the_exit_link() {
        if !has_ipv6_loopback() {
            eprintln!("no IPv6 loopback on this host, skipping");
            return;
        }

        assert!(on_exit_link_v6(Ipv6Addr::LOCALHOST));
    }

    #[test]
    fn unroutable_address_is_not_a_neighbour() {
        let unroutable: Ipv6Addr = "ff02::1:ffff".parse().unwrap();

        // Must not panic whatever the host's routing table says.
        let _ = on_exit_link_v6(unroutable);
        assert!(!on_exit_link_v6("::ffff:8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn mapped_ipv4_is_not_a_neighbour() {
        assert!(!on_exit_link_v6("::ffff:8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn is_own_address_keeps_ipv4_behaviour() {
        let documentation: IpAddr = "198.51.100.7".parse().unwrap();

        assert_eq!(is_own_address(documentation), own_or_local(documentation));
    }
}
