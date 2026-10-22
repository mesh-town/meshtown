//! Tests for `net/id.rs`, part two: text parsing, a point off the curve, serde.
//! Included from `id_tests.rs` as a child module; the helpers are from there as well.

use super::*;

mod parse {
    use super::*;

    mod hex_text {
        use super::*;

        #[test]
        fn lowercase_gives_the_same_id() {
            let parsed = RFC_PUBLIC_HEX
                .parse::<PeerId>()
                .expect("lowercase hex parses");
            assert_eq!(parsed, id_from_hex(RFC_PUBLIC_HEX));
        }

        #[test]
        fn display_and_parse_are_inverse() {
            for id in sample_ids() {
                assert_eq!(id.to_string().parse::<PeerId>().ok(), Some(id));
            }
        }

        #[test]
        fn uppercase_is_a_hex_error() {
            let err = RFC_PUBLIC_HEX.to_uppercase().parse::<PeerId>();
            assert!(
                matches!(err, Err(IdError::Hex)),
                "UPPERCASE hex is rejected: {err:?}"
            );
        }

        #[test]
        fn mixed_case_is_a_hex_error() {
            let text = format!("D{}", &RFC_PUBLIC_HEX[1..]);
            let err = text.parse::<PeerId>();
            assert!(
                matches!(err, Err(IdError::Hex)),
                "mixed case is rejected: {err:?}"
            );
        }

        #[test]
        fn non_hex_chars_of_length_64_are_a_hex_error() {
            let err = "g".repeat(64).parse::<PeerId>();
            assert!(
                matches!(err, Err(IdError::Hex)),
                "64 digits — hex only: {err:?}"
            );
        }
    }

    mod base32_text {
        use super::*;

        #[test]
        fn uppercase_without_padding_gives_the_same_id() {
            let text = base32_nopad_upper(&hex32(RFC_PUBLIC_HEX));
            assert_eq!(
                text.len(),
                52,
                "32 bytes = 52 base32 digits without padding"
            );
            assert_eq!(
                text.parse::<PeerId>().ok(),
                Some(id_from_hex(RFC_PUBLIC_HEX))
            );
        }

        #[test]
        fn lowercase_gives_the_same_id() {
            let text = base32_nopad_upper(&hex32(RFC_PUBLIC_HEX)).to_lowercase();
            assert_eq!(
                text.parse::<PeerId>().ok(),
                Some(id_from_hex(RFC_PUBLIC_HEX))
            );
        }

        #[test]
        fn mixed_case_gives_the_same_id() {
            let upper = base32_nopad_upper(&hex32(RFC_PUBLIC_HEX));
            let (head, tail) = upper.split_at(upper.len() / 2);
            let text = format!("{}{}", head.to_lowercase(), tail);
            assert_eq!(
                text.parse::<PeerId>().ok(),
                Some(id_from_hex(RFC_PUBLIC_HEX))
            );
        }

        #[test]
        fn padding_is_a_base32_error() {
            let text = format!("{}====", base32_nopad_upper(&hex32(RFC_PUBLIC_HEX)));
            let err = text.parse::<PeerId>();
            assert!(
                matches!(err, Err(IdError::Base32)),
                "no padding expected: {err:?}"
            );
        }

        #[test]
        fn symbol_outside_the_alphabet_is_a_base32_error() {
            let mut text = base32_nopad_upper(&hex32(RFC_PUBLIC_HEX));
            text.replace_range(10..11, "1");
            let err = text.parse::<PeerId>();
            assert!(
                matches!(err, Err(IdError::Base32)),
                "'1' is not in the base32 alphabet: {err:?}"
            );
        }

        #[test]
        fn fewer_than_32_bytes_is_a_length_error() {
            let text = base32_nopad_upper(&hex32(RFC_PUBLIC_HEX)[..31]);
            let err = text.parse::<PeerId>();
            assert!(
                matches!(err, Err(IdError::Length)),
                "31 bytes instead of 32: {err:?}"
            );
        }

        #[test]
        fn more_than_32_bytes_is_a_length_error() {
            let mut bytes = hex32(RFC_PUBLIC_HEX).to_vec();
            bytes.push(0);
            let err = base32_nopad_upper(&bytes).parse::<PeerId>();
            assert!(
                matches!(err, Err(IdError::Length)),
                "33 bytes instead of 32: {err:?}"
            );
        }
    }

    mod wrong_length {
        use super::*;

        #[test]
        fn text_of_63_chars_is_an_error() {
            let text = &RFC_PUBLIC_HEX[..63];
            assert!(text.parse::<PeerId>().is_err(), "63 digits — not an id");
        }

        #[test]
        fn text_of_65_chars_is_an_error() {
            let text = format!("{RFC_PUBLIC_HEX}a");
            assert!(text.parse::<PeerId>().is_err(), "65 digits — not an id");
        }

        #[test]
        fn empty_text_is_an_error() {
            assert!("".parse::<PeerId>().is_err(), "empty string — not an id");
        }
    }

    #[test]
    fn hex_of_a_point_off_the_curve_is_a_not_on_curve_error() {
        let text = to_hex(&off_curve_bytes());
        let err = text.parse::<PeerId>();
        assert!(matches!(err, Err(IdError::NotOnCurve)), "{err:?}");
    }
}

mod off_curve_point {
    use super::*;

    #[test]
    fn some_repeated_byte_pattern_is_rejected_by_the_library() {
        assert!(
            first_off_curve_pattern().is_some(),
            "a point off the curve is needed among [i; 32], i = 0..=255"
        );
    }

    #[test]
    fn from_bytes_rejects_it_with_not_on_curve() {
        let err = PeerId::from_bytes(&off_curve_bytes());
        assert!(matches!(err, Err(IdError::NotOnCurve)), "{err:?}");
    }
}

mod serde_forms {
    use super::*;

    #[test]
    fn json_is_a_string_with_the_lowercase_hex() {
        let id = id_from_hex(RFC_PUBLIC_HEX);
        let json = serde_json::to_string(&id).expect("json is written");
        assert_eq!(json, format!("\"{RFC_PUBLIC_HEX}\""));
    }

    #[test]
    fn json_round_trip_gives_the_same_id() {
        for id in sample_ids() {
            let json = serde_json::to_string(&id).expect("json is written");
            assert_eq!(serde_json::from_str::<PeerId>(&json).ok(), Some(id));
        }
    }

    #[test]
    fn json_with_uppercase_hex_is_an_error() {
        let json = format!("\"{}\"", RFC_PUBLIC_HEX.to_uppercase());
        assert!(
            serde_json::from_str::<PeerId>(&json).is_err(),
            "UPPERCASE hex in json is rejected"
        );
    }

    #[test]
    fn json_with_a_point_off_the_curve_is_an_error() {
        let json = format!("\"{}\"", to_hex(&off_curve_bytes()));
        assert!(
            serde_json::from_str::<PeerId>(&json).is_err(),
            "a point off the curve is rejected"
        );
    }

    #[test]
    fn postcard_is_exactly_the_32_bytes_of_the_id() {
        let id = id_from_hex(RFC_PUBLIC_HEX);
        let bytes = postcard::to_stdvec(&id).expect("postcard is written");
        assert_eq!(bytes.len(), 32, "no length prefix");
        assert_eq!(bytes.as_slice(), id.as_bytes().as_slice());
    }

    #[test]
    fn postcard_round_trip_gives_the_same_id() {
        for id in sample_ids() {
            let bytes = postcard::to_stdvec(&id).expect("postcard is written");
            assert_eq!(postcard::from_bytes::<PeerId>(&bytes).ok(), Some(id));
        }
    }

    #[test]
    fn postcard_with_a_point_off_the_curve_is_an_error() {
        let bytes = off_curve_bytes();
        assert!(
            postcard::from_bytes::<PeerId>(&bytes).is_err(),
            "parsing goes through from_bytes"
        );
    }

    #[test]
    fn postcard_shorter_than_32_bytes_is_an_error() {
        let bytes = hex32(RFC_PUBLIC_HEX);
        assert!(
            postcard::from_bytes::<PeerId>(&bytes[..31]).is_err(),
            "31 bytes — truncated"
        );
    }
}
