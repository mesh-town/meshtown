//! Tests of the `meshtown/port/1` protocol frame (`frame.rs`).

use super::*;
use crate::ports_sharing::types::Proto;

const CODES: [u8; 6] = [
    PORT_CODE_OK,
    PORT_CODE_NOT_PUBLISHED,
    PORT_CODE_CONNECT_FAILED,
    PORT_CODE_REFUSED,
    PORT_CODE_BAD_REQUEST,
    PORT_CODE_BUSY,
];

#[test]
fn request_roundtrip_and_layout() {
    let req = PortRequest {
        proto: Proto::Tcp,
        port: 8080,
    };
    assert_eq!(encode_port_request(req), [1, 0x1F, 0x90]);
    assert_eq!(parse_port_request(&encode_port_request(req)).unwrap(), req);
}

#[test]
fn reply_roundtrip() {
    for code in CODES {
        assert_eq!(parse_port_reply(&encode_port_reply(code)).unwrap(), code);
    }
}

#[test]
fn parse_rejects() {
    assert!(parse_port_request(&[3, 0x1F, 0x90]).is_err());
    assert!(parse_port_reply(&[2, 0]).is_err());
    assert!(parse_port_request(&[1, 0x1F]).is_err());
    assert!(parse_port_request(&[1, 0x1F, 0x90, 0]).is_err());
    assert!(parse_port_reply(&[1]).is_err());
    assert!(parse_port_reply(&[1, 0, 0]).is_err());
    assert!(parse_port_request(&[1, 0, 0]).is_err());
}

#[test]
fn parse_reply_passes_unknown_code() {
    assert_eq!(parse_port_reply(&[1, 9]).unwrap(), 9);
}

#[test]
fn code_texts_distinct() {
    let unknown = port_code_text(9);
    let texts: Vec<&str> = CODES.iter().map(|c| port_code_text(*c)).collect();
    for (i, t) in texts.iter().enumerate() {
        assert!(!t.is_empty());
        assert_ne!(*t, unknown);
        assert!(texts[i + 1..].iter().all(|o| o != t), "duplicate text {t}");
    }
}

fn req(proto: Proto, port: u16) -> PortRequest {
    PortRequest { proto, port }
}

mod request_v1 {
    use super::*;

    #[test]
    fn tcp_request_is_three_bytes_with_version_1() {
        // given: a TCP request for port 8080
        let bytes = encode_port_request(req(Proto::Tcp, 8080));
        // then: version 1, the port most significant byte first
        assert_eq!(bytes, [1, 0x1F, 0x90]);
        assert_eq!(bytes.len(), 3);
        assert_eq!(parse_port_request(&bytes).unwrap(), req(Proto::Tcp, 8080));
    }

    #[test]
    fn ports_1_and_65535_round_trip() {
        for port in [1u16, 65535] {
            let bytes = encode_port_request(req(Proto::Tcp, port));
            assert_eq!(parse_port_request(&bytes).unwrap(), req(Proto::Tcp, port));
        }
        assert_eq!(encode_port_request(req(Proto::Tcp, 65535)), [1, 0xFF, 0xFF]);
        assert_eq!(encode_port_request(req(Proto::Tcp, 1)), [1, 0, 1]);
    }

    #[test]
    fn port_0_is_rejected() {
        assert!(parse_port_request(&[1, 0, 0]).is_err());
    }

    #[test]
    fn wrong_length_is_rejected() {
        assert!(parse_port_request(&[]).is_err());
        assert!(parse_port_request(&[1]).is_err());
        assert!(parse_port_request(&[1, 0x1F]).is_err());
        assert!(parse_port_request(&[1, 0x1F, 0x90, 0]).is_err());
    }
}

mod request_v2 {
    use super::*;

    #[test]
    fn udp_request_is_four_bytes_with_version_2_and_proto_1() {
        // given: a UDP request for port 53
        let bytes = encode_port_request(req(Proto::Udp, 53));
        // then: version 2, protocol byte 1, then the port
        assert_eq!(bytes, [2, 1, 0, 53]);
        assert_eq!(parse_port_request(&bytes).unwrap(), req(Proto::Udp, 53));
    }

    #[test]
    fn udp_ports_1_and_65535_round_trip() {
        for port in [1u16, 65535] {
            let bytes = encode_port_request(req(Proto::Udp, port));
            assert_eq!(bytes.len(), 4);
            assert_eq!(parse_port_request(&bytes).unwrap(), req(Proto::Udp, port));
        }
        assert_eq!(
            encode_port_request(req(Proto::Udp, 65535)),
            [2, 1, 0xFF, 0xFF]
        );
    }

    #[test]
    fn proto_byte_0_reads_as_tcp() {
        // a client may send TCP in the long form
        assert_eq!(
            parse_port_request(&[2, 0, 0x1F, 0x90]).unwrap(),
            req(Proto::Tcp, 8080)
        );
    }

    #[test]
    fn proto_byte_2_is_rejected() {
        assert!(parse_port_request(&[2, 2, 0, 53]).is_err());
        assert!(parse_port_request(&[2, 9, 0, 53]).is_err());
        assert!(parse_port_request(&[2, 0xFF, 0, 53]).is_err());
    }

    #[test]
    fn port_0_is_rejected() {
        assert!(parse_port_request(&[2, 1, 0, 0]).is_err());
        assert!(parse_port_request(&[2, 0, 0, 0]).is_err());
    }

    #[test]
    fn wrong_length_is_rejected() {
        assert!(parse_port_request(&[2, 1, 0]).is_err());
        assert!(parse_port_request(&[2, 1, 0, 53, 0]).is_err());
        assert!(parse_port_request(&[2]).is_err());
    }
}

mod request_version {
    use super::*;

    #[test]
    fn version_3_is_rejected_in_both_lengths() {
        assert!(parse_port_request(&[3, 0, 0]).is_err());
        assert!(parse_port_request(&[3, 1, 0, 53]).is_err());
    }

    #[test]
    fn version_0_is_rejected_in_both_lengths() {
        assert!(parse_port_request(&[0, 0, 80]).is_err());
        assert!(parse_port_request(&[0, 0, 0, 80]).is_err());
    }

    #[test]
    fn version_1_with_four_bytes_is_rejected() {
        assert!(parse_port_request(&[1, 1, 0, 53]).is_err());
    }
}

mod request_tail {
    use super::*;

    #[test]
    fn version_1_head_has_no_tail() {
        let bytes = encode_port_request(req(Proto::Tcp, 8080));
        assert_eq!(request_tail_len(&[bytes[0], bytes[1], bytes[2]]), 0);
    }

    #[test]
    fn version_2_head_has_one_more_byte() {
        let bytes = encode_port_request(req(Proto::Udp, 53));
        assert_eq!(request_tail_len(&[bytes[0], bytes[1], bytes[2]]), 1);
    }

    #[test]
    fn foreign_versions_have_no_tail() {
        assert_eq!(request_tail_len(&[0, 0, 80]), 0);
        assert_eq!(request_tail_len(&[3, 1, 0]), 0);
        assert_eq!(request_tail_len(&[0xFF, 0xFF, 0xFF]), 0);
    }

    #[test]
    fn head_plus_tail_parses_back_for_each_protocol() {
        for proto in Proto::ALL {
            let bytes = encode_port_request(req(proto, 8080));
            let head = [bytes[0], bytes[1], bytes[2]];
            assert_eq!(3 + request_tail_len(&head), bytes.len(), "{proto:?}");
        }
    }
}

mod old_node {
    use super::*;

    /// What a node that knows only version 1 does with the first three bytes of a request.
    fn version_1_reader_accepts(head: &[u8]) -> bool {
        head.len() == 3 && head[0] == PORT_FRAME_VERSION
    }

    #[test]
    fn udp_request_head_fails_a_version_1_reader() {
        // given: UDP requests for ports of every high-byte class
        for port in [1u16, 53, 256, 8080, 65535] {
            let bytes = encode_port_request(req(Proto::Udp, port));
            // when: a version 1 reader takes the first three bytes
            let head = &bytes[..3];
            // then: it refuses (and the old server answers "bad request")
            assert!(!version_1_reader_accepts(head), "port {port}");
            assert_ne!(bytes[0], PORT_FRAME_VERSION);
        }
    }

    #[test]
    fn tcp_request_is_still_read_by_a_version_1_reader() {
        let bytes = encode_port_request(req(Proto::Tcp, 8080));
        assert!(version_1_reader_accepts(&bytes));
    }
}

mod busy_code {
    use super::*;

    #[test]
    fn code_is_7_and_names_the_session_limit() {
        assert_eq!(PORT_CODE_BUSY, 7);
        assert_eq!(port_code_text(PORT_CODE_BUSY), "too many UDP sessions");
    }

    #[test]
    fn reply_with_busy_code_round_trips() {
        assert_eq!(
            parse_port_reply(&encode_port_reply(PORT_CODE_BUSY)).unwrap(),
            7
        );
        assert_eq!(encode_port_reply(PORT_CODE_BUSY), [1, 7]);
    }

    #[test]
    fn frame_version_for_the_proto_form_is_2() {
        assert_eq!(PORT_FRAME_VERSION, 1);
        assert_eq!(PORT_FRAME_VERSION_PROTO, 2);
    }
}
