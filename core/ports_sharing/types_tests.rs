//! Tests of the protocol table `Proto` (`types.rs`).

use super::*;

mod names {
    use super::*;

    #[test]
    fn each_protocol_is_parsed_back_from_its_name() {
        // given: every protocol
        for proto in Proto::ALL {
            // when: it is turned into text and parsed
            let back = Proto::parse(proto.as_str());
            // then: the same protocol comes back
            assert_eq!(back, Some(proto), "protocol {proto:?}");
        }
    }

    #[test]
    fn names_are_lowercase_tcp_and_udp() {
        assert_eq!(Proto::Tcp.as_str(), "tcp");
        assert_eq!(Proto::Udp.as_str(), "udp");
    }

    #[test]
    fn all_lists_both_protocols_in_discriminant_order() {
        assert_eq!(Proto::ALL, [Proto::Tcp, Proto::Udp]);
    }

    #[test]
    fn parse_accepts_only_exact_lowercase_names() {
        for bad in [
            "TCP", "Udp", "UDP", "", " tcp", "tcp ", "sctp", "tcp/udp", "0", "1",
        ] {
            assert_eq!(Proto::parse(bad), None, "text {bad:?}");
        }
    }

    #[test]
    fn default_protocol_is_tcp() {
        assert_eq!(Proto::default(), Proto::Tcp);
    }
}

mod frame_byte {
    use super::*;

    #[test]
    fn zero_is_tcp_one_is_udp_two_is_unknown() {
        assert_eq!(Proto::from_wire(0), Some(Proto::Tcp));
        assert_eq!(Proto::from_wire(1), Some(Proto::Udp));
        assert_eq!(Proto::from_wire(2), None);
    }

    #[test]
    fn every_other_byte_is_unknown() {
        for b in 2..=u8::MAX {
            assert_eq!(Proto::from_wire(b), None, "byte {b}");
        }
    }

    #[test]
    fn wire_byte_round_trips_for_each_protocol() {
        for proto in Proto::ALL {
            assert_eq!(Proto::from_wire(proto.wire()), Some(proto));
        }
        assert_eq!((Proto::Tcp.wire(), Proto::Udp.wire()), (0, 1));
    }
}

mod pick {
    use super::*;

    #[test]
    fn tcp_takes_the_first_value() {
        assert_eq!(Proto::Tcp.pick(1, 2), 1);
    }

    #[test]
    fn udp_takes_the_second_value() {
        assert_eq!(Proto::Udp.pick(1, 2), 2);
    }
}

mod tcp_check {
    use super::*;

    #[test]
    fn only_tcp_is_tcp() {
        assert!(Proto::Tcp.is_tcp());
        assert!(!Proto::Udp.is_tcp());
    }
}

mod ordering {
    use super::*;

    #[test]
    fn tcp_sorts_before_udp() {
        // given: a list in reverse order
        let mut list = vec![
            (Proto::Udp, 53),
            (Proto::Tcp, 80),
            (Proto::Udp, 1),
            (Proto::Tcp, 53),
        ];
        // when: it is sorted
        list.sort_unstable();
        // then: protocol first, then port
        assert_eq!(
            list,
            [
                (Proto::Tcp, 53),
                (Proto::Tcp, 80),
                (Proto::Udp, 1),
                (Proto::Udp, 53)
            ]
        );
    }
}

mod serialization {
    use super::*;

    #[test]
    fn serializes_as_lowercase_text() {
        assert_eq!(serde_json::to_string(&Proto::Tcp).unwrap(), "\"tcp\"");
        assert_eq!(serde_json::to_string(&Proto::Udp).unwrap(), "\"udp\"");
    }
}
