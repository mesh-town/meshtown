use super::*;

#[test]
fn host_refuses_forbidden_and_noncanonical() {
    for bad in ["a b", "a%41", "127.1", "[::1", "a<b"] {
        assert!(host_to_ascii(bad).is_err(), "host_to_ascii: {bad:?}");
        let url = format!("http://{bad}/");
        assert!(host(&url).is_err(), "host: {url:?}");
    }
}

#[test]
fn host_refuses_empty_host() {
    for bad in ["http://", "https://:8080/", "http://u@/"] {
        assert!(host(bad).is_err(), "should be refused: {bad:?}");
    }
}

mod idn {
    use super::*;

    #[test]
    fn each_label_becomes_punycode() {
        assert_eq!(
            host_to_ascii("b\u{FC}cher.example").unwrap(),
            "xn--bcher-kva.example"
        );
        assert_eq!(
            host_to_ascii("\u{4F8B}\u{3048}.\u{30C6}\u{30B9}\u{30C8}").unwrap(),
            "xn--r8jz45g.xn--zckzah"
        );
    }

    #[test]
    fn ideographic_full_stop_separates_labels() {
        assert_eq!(
            host_to_ascii("\u{4F8B}\u{3048}\u{3002}\u{30C6}\u{30B9}\u{30C8}").unwrap(),
            "xn--r8jz45g.xn--zckzah"
        );
    }

    #[test]
    fn fullwidth_latin_and_upper_case_are_folded() {
        assert_eq!(
            host_to_ascii("\u{FF25}\u{FF38}\u{FF21}\u{FF2D}\u{FF30}\u{FF2C}\u{FF25}.com").unwrap(),
            "example.com"
        );
        assert_eq!(
            host_to_ascii("B\u{DC}CHER.Example").unwrap(),
            "xn--bcher-kva.example"
        );
    }

    #[test]
    fn rfc3492_chinese_sample_in_a_host() {
        let h = host("https://\u{4ED6}\u{4EEC}\u{4E3A}\u{4EC0}\u{4E48}\u{4E0D}\u{8BF4}\u{4E2D}\u{6587}.example/").unwrap();
        assert_eq!(h, "xn--ihqwcrb4cv8a8dqg056pqjye.example");
    }
}

mod ipv6 {
    use super::*;

    fn host_of(literal: &str) -> String {
        host(&format!("http://{literal}:8080/")).unwrap()
    }

    #[test]
    fn loopback_stays_compressed() {
        assert_eq!(host_of("[::1]"), "[::1]");
    }

    #[test]
    fn embedded_ipv4_becomes_two_hex_groups() {
        assert_eq!(host_of("[0:0::ffff:1.2.3.4]"), "[::ffff:102:304]");
    }

    #[test]
    fn longest_zero_run_is_compressed() {
        assert_eq!(host_of("[1:0:0:2:0:0:0:3]"), "[1:0:0:2::3]");
        assert_eq!(host_of("[0:0:0:0:0:0:0:0]"), "[::]");
    }

    #[test]
    fn first_of_equal_zero_runs_is_compressed() {
        assert_eq!(host_of("[1:0:0:2:0:0:3:4]"), "[1::2:0:0:3:4]");
    }

    #[test]
    fn single_zero_group_is_not_compressed() {
        assert_eq!(host_of("[1:0:2:3:4:5:6:7]"), "[1:0:2:3:4:5:6:7]");
    }

    #[test]
    fn upper_case_digits_are_lowered() {
        assert_eq!(host_of("[2001:DB8::A]"), "[2001:db8::a]");
    }
}

mod ascii {
    use super::*;

    #[test]
    fn host_is_lowercased_and_keeps_trailing_dot() {
        assert_eq!(
            host("HTTPS://Www.Example.ORG./x").unwrap(),
            "www.example.org."
        );
    }

    #[test]
    fn canonical_ipv4_passes() {
        assert_eq!(host("http://93.184.216.34:80/").unwrap(), "93.184.216.34");
    }
}
