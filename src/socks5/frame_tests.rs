//! Tests for parsing a SOCKS5 request and building the reply (`frame.rs`).

use super::*;

#[test]
fn domain_request_is_parsed() {
    let raw = encode_socks5_request(&Target::Domain("Example.COM".to_string()), 443).unwrap();
    let req = parse_socks5_request(&raw).unwrap();
    assert_eq!(req.target, Target::Domain("example.com".to_string()));
    assert_eq!(req.port, 443);
}

#[test]
fn ipv4_request_is_parsed() {
    let raw = encode_socks5_request(&Target::Ip(Ipv4Addr::new(93, 184, 216, 34)), 80).unwrap();
    let req = parse_socks5_request(&raw).unwrap();
    assert_eq!(req.target, Target::Ip(Ipv4Addr::new(93, 184, 216, 34)));
    assert_eq!(req.port, 80);
}

#[test]
fn wrong_version_command_and_address_kind_are_rejected() {
    // The BIND command — code 7.
    let mut bind = encode_socks5_request(&Target::Domain("example.com".to_string()), 80).unwrap();
    bind[1] = 2;
    let err = parse_socks5_request(&bind).unwrap_err();
    assert_eq!(err, RequestError::Command(2));
    assert_eq!(err.reply_code(), REP_CMD_NOT_SUPPORTED);
    // An IPv6 address — code 8.
    let mut six = vec![VERSION, CMD_CONNECT, 0, ATYP_IPV6];
    six.extend_from_slice(&[0u8; 18]);
    let err = parse_socks5_request(&six).unwrap_err();
    assert_eq!(err, RequestError::AddressKind(ATYP_IPV6));
    assert_eq!(err.reply_code(), REP_ATYP_NOT_SUPPORTED);
    // Version 4 — a general refusal.
    let mut four = encode_socks5_request(&Target::Domain("example.com".to_string()), 80).unwrap();
    four[0] = 4;
    let err = parse_socks5_request(&four).unwrap_err();
    assert_eq!(err, RequestError::Version(4));
    assert_eq!(err.reply_code(), REP_GENERAL);
}

#[test]
fn short_and_empty_requests_are_rejected() {
    assert_eq!(parse_socks5_request(&[VERSION, CMD_CONNECT, 0]), Err(RequestError::Short));
    assert_eq!(
        parse_socks5_request(&[VERSION, CMD_CONNECT, 0, ATYP_DOMAIN, 0, 0, 0]),
        Err(RequestError::EmptyName)
    );
    assert_eq!(
        parse_socks5_request(&[VERSION, CMD_CONNECT, 0, ATYP_DOMAIN, 9, b'a']),
        Err(RequestError::Short)
    );
}

#[test]
fn reply_is_ten_bytes_with_code() {
    let reply = encode_socks5_reply(REP_HOST_UNREACHABLE);
    assert_eq!(reply.len(), 10);
    assert_eq!(reply[0], VERSION);
    assert_eq!(reply[1], REP_HOST_UNREACHABLE);
    assert_eq!(reply[3], ATYP_IPV4);
}

mod from_host {
    use super::*;

    #[test]
    fn from_host_ipv4_literal_is_ip() {
        assert_eq!(Target::from_host("1.2.3.4"), Some(Target::Ip(Ipv4Addr::new(1, 2, 3, 4))));
    }

    #[test]
    fn from_host_name_is_lowercased_domain() {
        assert_eq!(Target::from_host("Site.Test"), Some(Target::Domain("site.test".to_string())));
    }

    #[test]
    fn from_host_empty_is_none() {
        assert_eq!(Target::from_host(""), None);
    }

    #[test]
    fn from_host_ipv6_is_none() {
        assert_eq!(Target::from_host("[::1]"), None);
    }

    #[test]
    fn from_host_name_over_max_len_is_none() {
        let long = "a".repeat(MAX_NAME_LEN + 1);
        assert_eq!(Target::from_host(&long), None);
    }

    #[test]
    fn from_host_name_of_max_len_is_domain() {
        let edge = "a".repeat(MAX_NAME_LEN);
        assert_eq!(Target::from_host(&edge), Some(Target::Domain(edge)));
    }
}
