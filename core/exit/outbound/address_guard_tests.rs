//! Tests for the exit address filter (`address_guard.rs`): the IPv4 table, "closed by default" for
//! IPv6, and the node's own address probe. Network boundaries are checked
//! in pairs "last closed — first open neighbor", so that a prefix shift doesn't go unnoticed.

use super::*;

mod v4 {
    use super::*;

    #[test]
    fn loopback_v4_is_forbidden() {
        assert_all_forbidden(
            &[
                "127.0.0.0",
                "127.0.0.1",
                "127.255.255.255",
                "0.0.0.0",
                "0.255.255.255",
            ],
            "loopback — the exit's own services",
        );
    }

    #[test]
    fn private_v4_nets_are_forbidden() {
        assert_all_forbidden(
            &[
                "10.0.0.0",
                "10.255.255.255",
                "172.16.0.0",
                "172.31.255.255",
                "192.168.0.0",
                "192.168.255.255",
                "100.64.0.0",
                "100.127.255.255",
            ],
            "the exit owner's private network",
        );
    }

    #[test]
    fn service_v4_nets_are_forbidden() {
        assert_all_forbidden(
            &[
                "169.254.0.0",
                "169.254.255.255",
                "192.0.0.0",
                "192.0.0.255",
                "192.0.2.0",
                "192.0.2.255",
                "198.18.0.0",
                "198.19.255.255",
                "198.51.100.0",
                "198.51.100.255",
                "203.0.113.0",
                "203.0.113.255",
                "224.0.0.0",
                "239.255.255.255",
                "240.0.0.0",
                "255.255.255.255",
            ],
            "service networks: link-local, documentation, test benches, multicast, reserved",
        );
    }

    #[test]
    fn v4_neighbours_outside_closed_nets_are_open() {
        assert_all_open(
            &[
                "1.0.0.0",
                "9.255.255.255",
                "11.0.0.0",
                "100.63.255.255",
                "100.128.0.0",
                "126.255.255.255",
                "128.0.0.0",
                "169.253.255.255",
                "169.255.0.0",
                "172.15.255.255",
                "172.32.0.0",
                "191.255.255.255",
                "192.0.1.0",
                "192.0.1.255",
                "192.0.3.0",
                "192.167.255.255",
                "192.169.0.0",
                "198.17.255.255",
                "198.20.0.0",
                "198.51.99.255",
                "198.51.101.0",
                "203.0.112.255",
                "203.0.114.0",
                "223.255.255.255",
            ],
            "neighbor of a closed network just outside its boundary",
        );
    }

    #[test]
    fn public_v4_is_open() {
        assert_all_open(
            &["8.8.8.8", "1.1.1.1", "93.184.216.34"],
            "an ordinary public address",
        );
    }
}

mod v6 {
    use super::*;

    #[test]
    fn v6_loopback_and_unspecified_are_forbidden() {
        assert_all_forbidden(&["::1", "::"], "loopback and unspecified — the exit's own services");
    }

    #[test]
    fn v6_unique_local_is_forbidden() {
        assert_all_forbidden(
            &["fc00::1", "fd00::1", "fdff:ffff::1"],
            "unique local fc00::/7 — the owner's private network",
        );
    }

    #[test]
    fn v6_link_local_and_site_local_are_forbidden() {
        assert_all_forbidden(
            &["fe80::1", "febf::1", "fec0::1"],
            "link-local and site-local — the owner's local network",
        );
    }

    #[test]
    fn v6_multicast_is_forbidden() {
        assert_all_forbidden(&["ff02::1", "ff0e::1"], "multicast — not a site address");
    }

    #[test]
    fn v4_mapped_follows_embedded_v4() {
        assert_all_forbidden(
            &["::ffff:127.0.0.1"],
            "loopback wrapped as IPv4-mapped — the exit's own services",
        );
        assert_all_forbidden(
            &["::ffff:192.168.1.1", "::ffff:10.0.0.1"],
            "the owner's private network wrapped as IPv4-mapped",
        );
        assert_all_open(
            &["::ffff:8.8.8.8"],
            "a public IPv4 wrapped as IPv4-mapped is judged as IPv4",
        );
    }

    #[test]
    fn v4_compatible_follows_embedded_v4() {
        assert_all_forbidden(&["::192.168.1.1"], "the owner's private network in the ::a.b.c.d form");
        assert_all_open(&["::8.8.8.8"], "a public IPv4 in the ::a.b.c.d form");
    }

    #[test]
    fn nat64_and_6to4_are_forbidden() {
        assert_all_forbidden(
            &[
                "64:ff9b::c0a8:101",
                "64:ff9b::808:808",
                "2002:c0a8:101::",
                "2002:808:808::",
            ],
            "NAT64 and 6to4 carry IPv4 inside — the exit doesn't open them wholesale",
        );
    }

    #[test]
    fn teredo_and_documentation_v6_are_forbidden() {
        assert_all_forbidden(
            &["2001::1", "2001:db8::1", "3fff::1"],
            "Teredo and documentation networks inside 2000::/3",
        );
    }

    #[test]
    fn v6_outside_global_unicast_is_forbidden() {
        assert_all_forbidden(
            &[
                "1fff:ffff::1",
                "4000::1",
                "100::1",
                // IPv4-translated: `to_ipv4` doesn't take it, and it falls out of 2000::/3 too.
                "::ffff:0:7f00:1",
            ],
            "outside global unicast 2000::/3",
        );
    }

    #[test]
    fn v6_global_unicast_is_open() {
        assert_all_open(
            &[
                "2606:4700:4700::1111",
                "2a00:1450:4001::1",
                "2000::1",
                // A Teredo neighbor just outside its /32.
                "2001:4860::1",
            ],
            "global unicast outside closed networks",
        );
    }
}

mod own {
    use super::*;

    #[test]
    fn loopback_is_own_address() {
        assert!(is_own_address(ip("127.0.0.1")), "loopback is the node's own address");
        assert!(
            !is_own_address(ip("192.0.2.1")),
            "192.0.2.1 (TEST-NET-1) is not held by the node — a foreign address"
        );
    }

    #[test]
    fn outgoing_source_is_own_address() {
        let Some(src) = outgoing_source() else {
            eprintln!("no default route — outgoing address probe skipped");
            return;
        };

        assert!(
            is_own_address(src),
            "{src}: the address the node goes out from is its own"
        );
    }
}

/// Parses an address from text; on error the panic names the string.
fn ip(text: &str) -> IpAddr {
    text.parse::<IpAddr>().expect(text)
}

/// Every address in the list is closed; on failure it shows which one and why it must be closed.
fn assert_all_forbidden(list: &[&str], why: &str) {
    for text in list {
        assert!(is_forbidden(ip(text)), "{text} must be closed: {why}");
    }
}

/// Every address in the list is open; on failure it shows which one and why it must be open.
fn assert_all_open(list: &[&str], why: &str) {
    for text in list {
        assert!(!is_forbidden(ip(text)), "{text} must be open: {why}");
    }
}

/// The address this host goes out from: UDP `connect` only picks a route and sends no packets.
/// An error (no default route) gives `None`.
fn outgoing_source() -> Option<IpAddr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    sock.local_addr().ok().map(|addr| addr.ip())
}
