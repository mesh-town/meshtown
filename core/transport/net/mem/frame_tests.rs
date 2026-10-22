//! Tests of `mem` frames (`frame.rs`): byte layout, round trip, parse errors, the UDX marker.

use super::*;
use bytes::Bytes;

mod round_trip {
    use super::*;

    #[test]
    fn hello_keeps_the_service_name() {
        assert_round_trip(Frame::Hello(bytes(b"meshtown/auth/1")));
    }

    #[test]
    fn hello_of_one_byte_survives() {
        assert_round_trip(Frame::Hello(bytes(b"x")));
    }

    #[test]
    fn accept_survives() {
        assert_round_trip(Frame::Accept);
    }

    #[test]
    fn refuse_survives() {
        assert_round_trip(Frame::Refuse);
    }

    #[test]
    fn open_with_the_largest_stream_id_survives() {
        assert_round_trip(Frame::Open(u32::MAX));
    }

    #[test]
    fn data_with_a_payload_survives() {
        assert_round_trip(Frame::Data(3, bytes(b"payload")));
    }

    #[test]
    fn data_with_the_largest_id_and_an_empty_payload_survives() {
        assert_round_trip(Frame::Data(u32::MAX, Bytes::new()));
    }

    #[test]
    fn data_holding_every_byte_value_survives() {
        let all: Vec<u8> = (0..=255).collect();

        assert_round_trip(Frame::Data(1, Bytes::from(all)));
    }

    #[test]
    fn fin_with_the_largest_stream_id_survives() {
        assert_round_trip(Frame::Fin(u32::MAX));
    }

    #[test]
    fn window_with_the_largest_numbers_survives() {
        assert_round_trip(Frame::Window(u32::MAX, u32::MAX));
    }

    #[test]
    fn close_with_a_reason_survives() {
        assert_round_trip(Frame::Close {
            code: 7,
            reason: bytes(b"bye"),
        });
    }

    #[test]
    fn close_with_the_largest_code_and_an_empty_reason_survives() {
        assert_round_trip(Frame::Close {
            code: u32::MAX,
            reason: Bytes::new(),
        });
    }
}

mod wire_layout {
    use super::*;

    #[test]
    fn hello_is_kind_01_then_the_service_bytes() {
        let wire = Frame::Hello(bytes(b"ab")).encode();

        assert_eq!(&wire[..], &[0x01, b'a', b'b']);
    }

    #[test]
    fn accept_is_the_single_byte_02() {
        assert_eq!(&Frame::Accept.encode()[..], &[0x02]);
    }

    #[test]
    fn refuse_is_the_single_byte_03() {
        assert_eq!(&Frame::Refuse.encode()[..], &[0x03]);
    }

    #[test]
    fn open_is_kind_10_then_the_big_endian_id() {
        let wire = Frame::Open(0x0102_0304).encode();

        assert_eq!(&wire[..], &[0x10, 1, 2, 3, 4]);
    }

    #[test]
    fn data_is_kind_11_then_the_id_then_the_payload() {
        let wire = Frame::Data(5, bytes(&[9, 8])).encode();

        assert_eq!(&wire[..], &[0x11, 0, 0, 0, 5, 9, 8]);
    }

    #[test]
    fn fin_is_kind_12_then_the_big_endian_id() {
        let wire = Frame::Fin(0x0102_0304).encode();

        assert_eq!(&wire[..], &[0x12, 1, 2, 3, 4]);
    }

    #[test]
    fn window_is_kind_13_then_the_id_then_the_increment() {
        let wire = Frame::Window(0x0102_0304, 0x0A0B_0C0D).encode();

        assert_eq!(&wire[..], &[0x13, 1, 2, 3, 4, 0x0A, 0x0B, 0x0C, 0x0D]);
    }

    #[test]
    fn close_is_kind_20_then_the_big_endian_code_then_the_reason() {
        let wire = Frame::Close {
            code: 0x0102_0304,
            reason: bytes(b"bye"),
        }
        .encode();

        assert_eq!(&wire[..], &[0x20, 1, 2, 3, 4, b'b', b'y', b'e']);
    }
}

mod minimal_frames {
    use super::*;

    #[test]
    fn data_with_no_payload_bytes_decodes_to_an_empty_payload() {
        let got =
            Frame::decode(bytes(&[0x11, 0, 0, 0, 5])).expect("an empty Data is a valid frame");

        assert_eq!(got, Frame::Data(5, Bytes::new()));
    }

    #[test]
    fn close_with_no_reason_bytes_decodes_to_an_empty_reason() {
        let got = Frame::decode(bytes(&[0x20, 0, 0, 0, 7]))
            .expect("Close without a reason is a valid frame");

        assert_eq!(
            got,
            Frame::Close {
                code: 7,
                reason: Bytes::new()
            }
        );
    }
}

mod malformed_input {
    use super::*;

    #[test]
    fn an_empty_packet_is_an_error() {
        assert!(
            Frame::decode(Bytes::new()).is_err(),
            "an empty packet was accepted"
        );
    }

    #[test]
    fn every_unknown_kind_is_an_error() {
        let known = [0x01, 0x02, 0x03, 0x10, 0x11, 0x12, 0x13, 0x20];

        for kind in (0..=255u8).filter(|k| !known.contains(k)) {
            let mut wire = vec![kind];
            wire.extend_from_slice(&[0; 16]);

            assert!(
                Frame::decode(Bytes::from(wire)).is_err(),
                "unknown kind {kind:#04x} was accepted"
            );
        }
    }

    #[test]
    fn a_lone_kind_byte_of_a_frame_with_fields_is_an_error() {
        for kind in [0x10u8, 0x11, 0x12, 0x13, 0x20] {
            assert!(
                Frame::decode(bytes(&[kind])).is_err(),
                "a one-byte frame of kind {kind:#04x} was accepted"
            );
        }
    }

    #[test]
    fn every_proper_prefix_of_a_fixed_frame_is_an_error() {
        let frames = [
            Frame::Open(1),
            Frame::Data(1, Bytes::new()),
            Frame::Fin(1),
            Frame::Window(1, 2),
            Frame::Close {
                code: 1,
                reason: Bytes::new(),
            },
        ];

        for frame in &frames {
            let wire = frame.encode();
            for len in 1..wire.len() {
                assert!(
                    Frame::decode(wire.slice(..len)).is_err(),
                    "{frame:?} truncated to {len} bytes was accepted"
                );
            }
        }
    }

    #[test]
    fn extra_bytes_after_a_fixed_frame_are_an_error() {
        let frames = [
            Frame::Accept,
            Frame::Refuse,
            Frame::Open(1),
            Frame::Fin(1),
            Frame::Window(1, 2),
        ];

        for frame in &frames {
            let mut wire = frame.encode().to_vec();
            wire.push(0);

            assert!(
                Frame::decode(Bytes::from(wire)).is_err(),
                "{frame:?} with an extra byte was accepted"
            );
        }
    }

    #[test]
    fn a_hello_without_a_service_name_is_an_error() {
        assert!(
            Frame::decode(bytes(&[0x01])).is_err(),
            "an empty Hello was accepted"
        );
    }

    #[test]
    fn the_error_has_a_readable_message() {
        let err = Frame::decode(Bytes::new()).expect_err("an empty packet must be an error");

        assert!(!err.to_string().is_empty(), "Display of the error is empty");
    }

    #[test]
    fn the_error_is_a_std_error() {
        fn takes_error<E: std::error::Error>(_: &E) {}
        let err = Frame::decode(Bytes::new()).expect_err("an empty packet must be an error");

        takes_error(&err);
    }
}

mod udx_mark {
    use super::*;

    #[test]
    fn no_frame_kind_starts_with_ff() {
        let frames = [
            Frame::Hello(bytes(b"svc")),
            Frame::Accept,
            Frame::Refuse,
            Frame::Open(u32::MAX),
            Frame::Data(u32::MAX, Bytes::new()),
            Frame::Fin(u32::MAX),
            Frame::Window(u32::MAX, u32::MAX),
            Frame::Close {
                code: u32::MAX,
                reason: Bytes::new(),
            },
        ];

        for frame in &frames {
            assert_ne!(
                frame.encode()[0],
                0xFF,
                "{frame:?} took the UDX marker 0xFF"
            );
        }
    }

    #[test]
    fn a_packet_starting_with_ff_is_not_a_frame() {
        assert!(
            Frame::decode(bytes(&[0xFF, 0, 0, 0, 1])).is_err(),
            "a packet with the UDX marker was accepted as a frame"
        );
    }
}

// ---- helpers ----

fn bytes(b: &[u8]) -> Bytes {
    Bytes::copy_from_slice(b)
}

fn assert_round_trip(frame: Frame) {
    let back = Frame::decode(frame.encode()).expect("the own frame must parse");

    assert_eq!(back, frame);
}
