use super::*;
use crate::transport::net::endpoint_shim::from_endpoint_addr;
use crate::transport::net::endpoint_shim::{EndpointAddr, RelayUrl, TransportAddr};
use crate::transport::net::NodeSecret;
use crate::transport::net::{AddrHint, PeerAddr, RelayRef};
use std::net::SocketAddr;

#[test]
fn peer_lines_and_parse_peer_line_roundtrip() {
    let id = NodeSecret::from_bytes(&[7u8; 32]).peer_id();
    let ipv4: SocketAddr = "203.0.113.5:4433".parse().unwrap();
    let ipv6: SocketAddr = "[2001:db8::1]:4433".parse().unwrap();
    let relay: RelayUrl = "https://relay.example.com".parse().unwrap();
    let addr = from_endpoint_addr(&EndpointAddr::from_parts(
        id,
        [
            TransportAddr::Ip(ipv4),
            TransportAddr::Ip(ipv6),
            TransportAddr::Relay(relay),
        ],
    ));
    let lines = peer_lines(&addr);
    assert!(!lines.is_empty());
    for line in &lines {
        let parsed = parse_peer_line(line).unwrap();
        assert_eq!(parsed.id, id);
    }
    assert!(lines.iter().any(|l| l.contains("203.0.113.5:4433")));
    assert!(lines.iter().any(|l| l.contains("2001:db8::1")));
    assert!(lines.iter().any(|l| l.contains("relay.example.com")));
}

#[test]
fn parse_peer_line_rejects_bad_input() {
    assert!(parse_peer_line("no-at-sign").is_err());
    assert!(parse_peer_line("@203.0.113.5:4433").is_err());
    assert!(parse_peer_line("not-an-id@203.0.113.5:4433").is_err());
    let id = NodeSecret::from_bytes(&[8u8; 32]).peer_id();
    assert!(parse_peer_line(&format!("{id}@not-an-address")).is_err());
    assert!(parse_peer_line(&format!("{id}@ftp://relay.example.com")).is_err());
}

const ID7: &str = "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c";

#[test]
fn peer_lines_output_is_literal() {
    let id = NodeSecret::from_bytes(&[7u8; 32]).peer_id();
    assert_eq!(id.to_string(), ID7);
    let addr = from_endpoint_addr(&EndpointAddr::from_parts(
        id,
        [
            TransportAddr::Ip("203.0.113.5:4433".parse().unwrap()),
            TransportAddr::Ip("[2001:db8::1]:4433".parse().unwrap()),
            TransportAddr::Relay("https://relay.example.com".parse().unwrap()),
        ],
    ));
    assert_eq!(
        peer_lines(&addr),
        vec![
            format!("{ID7}@203.0.113.5:4433"),
            format!("{ID7}@[2001:db8::1]:4433"),
            format!("{ID7}@https://relay.example.com/"),
        ]
    );
}

#[test]
fn parse_peer_line_error_texts_are_literal() {
    let err = |line: &str| parse_peer_line(line).unwrap_err();
    assert_eq!(err("no-at-sign"), "no @ separator");
    assert_eq!(err("@203.0.113.5:4433"), "no identifier before @");
    assert_eq!(
        err("not-an-id@203.0.113.5:4433"),
        "couldn't parse the identifier: a full node id is needed"
    );
    let bad_addr = "couldn't parse the address: an ip:port or a relay url is needed";
    assert_eq!(err(&format!("{ID7}@not-an-address")), bad_addr);
    assert_eq!(err(&format!("{ID7}@ftp://relay.example.com")), bad_addr);
    assert_eq!(err(&format!("{ID7}@http://exa mple")), bad_addr);
}

#[test]
fn parse_peer_line_results_are_literal() {
    let id = NodeSecret::from_bytes(&[7u8; 32]).peer_id();
    let relay = parse_peer_line(&format!("{ID7}@https://relay.example.com")).unwrap();
    assert_eq!(
        relay,
        PeerAddr {
            id,
            hints: vec![AddrHint::Relay(RelayRef::Url(
                "https://relay.example.com/".to_string()
            ))],
        }
    );
    let ip = parse_peer_line(&format!("  {ID7}@203.0.113.5:4433 ")).unwrap();
    assert_eq!(
        ip,
        PeerAddr {
            id,
            hints: vec![AddrHint::Ip("203.0.113.5:4433".parse().unwrap())]
        }
    );
}

#[test]
fn peer_lines_of_unsorted_hand_made_addr_equals_literal() {
    let id = NodeSecret::from_bytes(&[7u8; 32]).peer_id();
    let v4: SocketAddr = "203.0.113.5:4433".parse().unwrap();
    let v6: SocketAddr = "[2001:db8::1]:4433".parse().unwrap();
    let addr = PeerAddr {
        id,
        hints: vec![
            AddrHint::Relay(RelayRef::Url("https://relay.example.com/".to_string())),
            AddrHint::Ip(v6),
            AddrHint::Ip(v4),
            AddrHint::Ip(v4),
            AddrHint::Relay(RelayRef::Peer(id)),
            AddrHint::Opaque {
                kind: 1,
                bytes: vec![1, 2],
            },
        ],
    };
    assert_eq!(
        peer_lines(&addr),
        vec![
            format!("{ID7}@203.0.113.5:4433"),
            format!("{ID7}@[2001:db8::1]:4433"),
            format!("{ID7}@https://relay.example.com/"),
        ]
    );
}
