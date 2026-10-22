//! Tests of `invite.rs`: direct addrs order.

use super::*;

fn behind_bridges() -> PeerAddr {
    let raw = ["10.99.129.2", "10.20.30.2", "192.168.88.129"]
        .iter()
        .fold(EndpointAddr::new(id(1)), |a, ip| {
            a.with_ip_addr(SocketAddr::new(ip.parse().unwrap(), 54674))
        });
    from_endpoint_addr(&raw)
}

fn first_entry_addrs(primary: &PrimaryIps) -> Vec<SocketAddr> {
    let s = build(&head_made_at(NOW), &behind_bridges(), &[], primary).unwrap();
    crate::transport::net::endpoint_shim::to_endpoint_addr(&parse(&s).unwrap().peers[0])
        .ip_addrs()
        .copied()
        .collect()
}

mod with_route_and_bridges {
    use super::*;

    #[test]
    fn invite_carries_lan_address() {
        // given
        let primary = PrimaryIps {
            v4: Some("192.168.88.129".parse().unwrap()),
            v6: None,
        };
        // when
        let ips = first_entry_addrs(&primary);
        // then
        assert_eq!(ips.len(), 2, "{ips:?}");
        let en0: SocketAddr = "192.168.88.129:54674".parse().unwrap();
        assert!(ips.contains(&en0), "en0 lost on truncation: {ips:?}");
    }
}

mod without_route_info {
    use super::*;

    #[test]
    fn takes_first_two_by_number() {
        // given / when
        let ips = first_entry_addrs(&PrimaryIps::NONE);
        // then
        let want: Vec<SocketAddr> = vec![
            "10.20.30.2:54674".parse().unwrap(),
            "10.99.129.2:54674".parse().unwrap(),
        ];
        assert_eq!(ips, want);
    }
}

mod with_foreign_tunnel_address {
    use super::*;

    #[test]
    fn invite_drops_tunnel_when_public_address_present() {
        // given
        let me = node_without_relay(&[
            "10.99.0.2:1",
            "100.64.0.7:1",
            "192.168.1.2:1",
            "93.184.216.34:1",
        ]);
        // when
        let ips = first_entry_addrs_of(&me);
        // then
        let tunnel: SocketAddr = "100.64.0.7:1".parse().unwrap();
        assert_eq!(ips.len(), 2, "{ips:?}");
        assert!(
            !ips.contains(&tunnel),
            "tunnel address 100.64/10 stayed next to a public one: {ips:?}"
        );
    }

    #[test]
    fn invite_keeps_tunnel_last_for_tailnet_only_pair() {
        // given
        let me = node_without_relay(&[
            "10.99.0.2:1",
            "100.64.0.7:1",
            "192.168.1.2:1",
            "10.20.30.2:1",
        ]);
        // when
        let ips = first_entry_addrs_of(&me);
        // then
        let tunnel: SocketAddr = "100.64.0.7:1".parse().unwrap();
        assert_eq!(ips.len(), 2, "{ips:?}");
        assert_eq!(ips[1], tunnel, "tunnel lost without public ones: {ips:?}");
    }

    #[test]
    fn lone_tunnel_address_stays_in_invite() {
        // given
        let me = node_without_relay(&["100.64.0.7:1"]);
        // when
        let ips = first_entry_addrs_of(&me);
        // then
        let tunnel: SocketAddr = "100.64.0.7:1".parse().unwrap();
        assert_eq!(
            ips,
            vec![tunnel],
            "the only address of a node without relay is not removed"
        );
    }
}

/// Own address without relay with the listed IP addresses.
fn node_without_relay(ips: &[&str]) -> PeerAddr {
    let raw = ips.iter().fold(EndpointAddr::new(id(1)), |a, ip| {
        a.with_ip_addr(ip.parse().unwrap())
    });
    from_endpoint_addr(&raw)
}

/// Builds an invite without a default route and extracts the IP addresses of the first record.
fn first_entry_addrs_of(me: &PeerAddr) -> Vec<SocketAddr> {
    let s = build(&head_made_at(NOW), me, &[], &PrimaryIps::NONE).unwrap();
    crate::transport::net::endpoint_shim::to_endpoint_addr(&parse(&s).unwrap().peers[0])
        .ip_addrs()
        .copied()
        .collect()
}
