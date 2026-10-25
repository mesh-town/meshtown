use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::*;
use crate::proxy::socks5::{Target, UdpPacket};

fn packet(target: Target, port: u16) -> UdpPacket {
    UdpPacket {
        target,
        port,
        data: b"hello".to_vec(),
    }
}

fn frame_of(packet: UdpPacket) -> Frame {
    let bytes = to_gateway_frame(packet).expect("fits a frame");
    codec::parse_body(&bytes[2..]).expect("a valid frame")
}

mod conn_id {
    use super::*;

    #[test]
    fn ipv4_and_domain_use_0() {
        let v4 = frame_of(packet(Target::Ip(Ipv4Addr::new(192, 0, 2, 1).into()), 53));
        let name = frame_of(packet(Target::Domain("example.com".into()), 53));
        assert_eq!((v4.conn_id, name.conn_id), (0, 0));
        assert_eq!(
            name.target,
            Some(codec::Target::Domain("example.com".into(), 53))
        );
    }

    #[test]
    fn ipv6_uses_1() {
        let frame = frame_of(packet(
            Target::Ip(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1).into()),
            53,
        ));
        assert_eq!(frame.conn_id, 1);
        assert_eq!(frame.flags, codec::FLAG_DATA);
        assert_eq!(frame.data, b"hello");
    }

    #[test]
    fn mapped_ipv4_becomes_ipv4_on_0() {
        let mapped = Ipv4Addr::new(192, 0, 2, 7).to_ipv6_mapped();
        let frame = frame_of(packet(Target::Ip(IpAddr::V6(mapped)), 9));
        assert_eq!(frame.conn_id, 0);
        assert_eq!(
            frame.target,
            Some(codec::Target::Ip(SocketAddr::from(([192, 0, 2, 7], 9))))
        );
    }

    #[test]
    fn domain_holding_ipv6_literal_becomes_ip_on_1() {
        let frame = frame_of(packet(Target::Domain("2001:db8::1".into()), 9));
        assert_eq!(frame.conn_id, 1);
        assert_eq!(
            frame.target,
            Some(codec::Target::Ip("[2001:db8::1]:9".parse().unwrap()))
        );
    }
}

mod replies {
    use super::*;

    #[test]
    fn data_frame_gives_source_and_payload() {
        for from in ["192.0.2.1:53", "[2001:db8::1]:53"] {
            let from: SocketAddr = from.parse().unwrap();
            let reply = from_gateway_frame(Frame::data(0, from, b"x".to_vec()));
            assert_eq!(reply, Some((from, b"x".to_vec())));
        }
    }

    #[test]
    fn keepalive_and_error_frames_are_skipped() {
        assert_eq!(from_gateway_frame(Frame::keepalive(0)), None);
        assert_eq!(from_gateway_frame(Frame::error(0)), None);
        let mut with_error = Frame::data(0, "192.0.2.1:53".parse().unwrap(), vec![]);
        with_error.flags |= codec::FLAG_ERR;
        assert_eq!(from_gateway_frame(with_error), None);
    }

    #[test]
    fn oversize_packet_gives_none() {
        let big = UdpPacket {
            target: Target::Ip(Ipv4Addr::new(192, 0, 2, 1).into()),
            port: 1,
            data: vec![0; 70_000],
        };
        assert!(to_gateway_frame(big).is_none());
    }
}

mod gateway_targets {
    use super::*;

    #[test]
    fn ipv4_target_stays_ip() {
        let target = gateway_target(Target::Ip(Ipv4Addr::new(192, 0, 2, 1).into()), 53);
        assert_eq!(
            target,
            codec::Target::Ip(SocketAddr::from(([192, 0, 2, 1], 53)))
        );
    }

    #[test]
    fn ipv6_target_stays_ip() {
        let ip = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);
        let target = gateway_target(Target::Ip(ip.into()), 53);
        assert_eq!(target, codec::Target::Ip(SocketAddr::from((ip, 53))));
    }

    #[test]
    fn mapped_ipv6_becomes_ipv4() {
        let mapped = Ipv4Addr::new(192, 0, 2, 7).to_ipv6_mapped();
        let target = gateway_target(Target::Ip(IpAddr::V6(mapped)), 9);
        assert_eq!(
            target,
            codec::Target::Ip(SocketAddr::from(([192, 0, 2, 7], 9)))
        );
    }

    #[test]
    fn ip_literal_domain_becomes_ip() {
        let target = gateway_target(Target::Domain("10.0.0.1".into()), 80);
        assert_eq!(
            target,
            codec::Target::Ip(SocketAddr::from(([10, 0, 0, 1], 80)))
        );
    }

    #[test]
    fn name_stays_domain() {
        let target = gateway_target(Target::Domain("example.com".into()), 443);
        assert_eq!(target, codec::Target::Domain("example.com".into(), 443));
    }
}
