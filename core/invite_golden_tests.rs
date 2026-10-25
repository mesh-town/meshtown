//! Golden test of the `mesh-invite2-` invite string (format 3): the string is a literal recomputed outside the code
//! (`network_id` is `b3sum --derive-key 'meshtown network id v1' --length 4` over 32 bytes 0x09);
//! the RFC 8032 vector id is stored in it as record bytes. A separate neighbor file: `invite_tests.rs` is far over the ceiling.

use std::net::SocketAddr;

use crate::transport::net::endpoint_shim::from_endpoint_addr;
use crate::transport::net::{NodeSecret, PeerId};

use super::*;
use crate::transport::addr_order::PrimaryIps;

const NOW: u64 = 1_790_000_000;

fn id_of(seed_hex: &str) -> PeerId {
    let seed: [u8; 32] = data_encoding::HEXLOWER
        .decode(seed_hex.as_bytes())
        .expect("seed — hex")
        .try_into()
        .expect("seed is 32 bytes");
    NodeSecret::from_bytes(&seed).peer_id()
}

fn head() -> InviteHead {
    InviteHead {
        secret: InviteSecret::from_bytes([0xC7; 32]),
        network_id: NetworkKey::from_bytes([9u8; 32]).subkeys().network_id,
        label: None,
        made_unix: NOW,
        kind: NetKind::Peer,
    }
}

#[test]
fn golden_invite_string_is_literal_and_parses_vector_id() {
    const KEY1: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    const INVITE: &str =
        "mesh-invite2-aplk27cqadd4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pahxytkqmaoxlkmadavrbk35ks762pewibz2b3qxf462uyrsllycdjupob2rdiaacad7aaaadia7qd34jvig";
    let id = id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    // RFC 8032 §7.1 anchor
    assert_eq!(id.to_string(), KEY1);
    let addr: SocketAddr = "127.0.0.1:4000".parse().expect("address");
    let me = from_endpoint_addr(
        &crate::transport::net::endpoint_shim::EndpointAddr::new(id).with_ip_addr(addr),
    );

    let s = build_keyed(&head(), &me, &[], &PrimaryIps::NONE).expect("build");
    assert_eq!(s, INVITE);

    let parsed = parse(&s).expect("parse");
    assert_eq!(parsed.peers.len(), 1);
    assert_eq!(parsed.peers[0].id.to_string(), KEY1);
}

#[test]
fn golden_invite_with_relay_and_others_is_literal() {
    use crate::transport::net::{AddrHint, PeerAddr, RelayRef};
    let id_me = id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let id_other = id_of("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb");
    let ip = |s: &str| s.parse::<SocketAddr>().expect("address");
    let me = crate::transport::net::endpoint_shim::EndpointAddr::new(id_me)
        .with_relay_url("https://relay.example/".parse().expect("url"))
        .with_ip_addr(ip("203.0.113.5:4000"))
        .with_ip_addr(ip("[2001:db8::5]:4000"));
    let me = from_endpoint_addr(&me);
    let other = crate::transport::net::endpoint_shim::EndpointAddr::new(id_other)
        .with_ip_addr(ip("10.0.0.2:1"))
        .with_ip_addr(ip("203.0.113.9:1"))
        .with_ip_addr(ip("[2001:db8::9]:1"));
    let other = from_endpoint_addr(&other);
    let routed = PrimaryIps {
        v4: Some("192.0.2.1".parse().unwrap()),
        v6: None,
    };
    let s = build_keyed(&head(), &me, &[me.clone(), other], &routed).expect("build");
    const INVITE: &str =
        "mesh-invite2-aplk27cqadd4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pahxytkqmawxlkmadavrbk35ks762pewibz2b3qxf462uyrsllycdjupob2rdiarm2duoryhgorpf5zgk3dbpexgk6dbnvygyzjpacapprgvay6uaf6d5bbyswusw4fkoti3p26jzgbmz4xmjfumydgvl4jk6rtayaacadfqa4ijaeasaainxaaaaaaaaaaaaaaaaaaasama67cnkbq";
    assert_eq!(s, INVITE);
    let parsed = parse(&s).expect("parse");
    // order and set of hints of the old path: `me` kept a single relay, the foreign one has public IPs
    let want = vec![
        PeerAddr {
            id: id_me,
            hints: vec![AddrHint::Relay(RelayRef::Url(
                "https://relay.example/".to_string(),
            ))],
        },
        PeerAddr {
            id: id_other,
            hints: vec![
                AddrHint::Ip(ip("203.0.113.9:1")),
                AddrHint::Ip(ip("[2001:db8::9]:1")),
            ],
        },
    ];
    assert_eq!(parsed.peers, want);
}
