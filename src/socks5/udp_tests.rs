//! Tests for the UDP datagram header (`udp.rs`).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use super::*;

mod parse {
    use super::*;

    #[test]
    fn ipv4_domain_and_ipv6_round_trip() {
        let targets = [
            Target::Ip(Ipv4Addr::new(1, 1, 1, 1).into()),
            Target::Domain("dns.example".to_string()),
            Target::Ip("2001:db8::53".parse::<IpAddr>().unwrap()),
        ];
        for target in targets {
            let raw = encode_udp_request(&target, 53, b"payload").unwrap();
            let packet = parse_udp_datagram(&raw).unwrap();
            assert_eq!(
                packet,
                UdpPacket {
                    target,
                    port: 53,
                    data: b"payload".to_vec()
                }
            );
        }
    }

    #[test]
    fn fragment_is_rejected() {
        let mut raw = encode_udp_request(&Target::Domain("a.test".to_string()), 1, b"x").unwrap();
        raw[2] = 1;
        assert_eq!(parse_udp_datagram(&raw), Err(UdpHeaderError::Fragmented(1)));
    }

    #[test]
    fn short_header_is_rejected() {
        assert_eq!(parse_udp_datagram(&[0, 0, 0]), Err(UdpHeaderError::Short));
        assert_eq!(
            parse_udp_datagram(&[0, 0, 0, 1, 1, 1]),
            Err(UdpHeaderError::Short)
        );
    }

    #[test]
    fn unknown_address_kind_is_rejected() {
        let raw = [0, 0, 0, 2, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            parse_udp_datagram(&raw),
            Err(UdpHeaderError::AddressKind(2))
        );
    }

    #[test]
    fn empty_name_is_rejected() {
        assert_eq!(
            parse_udp_datagram(&[0, 0, 0, 3, 0, 0, 53]),
            Err(UdpHeaderError::EmptyName)
        );
    }

    #[test]
    fn empty_payload_is_allowed() {
        let raw = encode_udp_request(&Target::Ip(Ipv4Addr::LOCALHOST.into()), 9, b"").unwrap();
        assert!(parse_udp_datagram(&raw).unwrap().data.is_empty());
    }
}

mod encode {
    use super::*;

    #[test]
    fn reply_from_ipv6_uses_atyp_4() {
        let from: SocketAddr = "[2001:db8::1]:5353".parse().unwrap();
        let raw = encode_udp_reply(from, b"ok");
        assert_eq!(raw[3], 4);
        assert_eq!(raw.len(), 4 + 16 + 2 + 2);
        let packet = parse_udp_datagram(&raw).unwrap();
        assert_eq!(packet.target, Target::Ip(from.ip()));
        assert_eq!((packet.port, packet.data.as_slice()), (5353, &b"ok"[..]));
    }

    #[test]
    fn reply_from_ipv4_uses_atyp_1() {
        let from: SocketAddr = "192.0.2.7:53".parse().unwrap();
        let raw = encode_udp_reply(from, b"ok");
        assert_eq!(raw, [0, 0, 0, 1, 192, 0, 2, 7, 0, 53, b'o', b'k']);
    }

    #[test]
    fn name_over_255_bytes_is_an_error() {
        let target = Target::Domain("a".repeat(256));
        assert!(encode_udp_request(&target, 1, b"x").is_err());
    }
}
