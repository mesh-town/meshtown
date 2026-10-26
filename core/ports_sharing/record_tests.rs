//! Tests of the publication entry key and value (`record.rs`).

use super::*;
use crate::ports_sharing::types::Proto;
use crate::transport::net::{NodeSecret, PeerId};

fn id() -> PeerId {
    NodeSecret::from_bytes(&[7; 32]).peer_id()
}

fn other() -> PeerId {
    NodeSecret::from_bytes(&[8; 32]).peer_id()
}

fn rec(name: &str, audience: Audience) -> PortRecord {
    PortRecord {
        v: PORT_RECORD_VERSION,
        name: name.to_string(),
        audience,
    }
}

/// A check that was previously unverified: a `PeerId` in postcard is 32 bytes without a length,
/// not an ed25519 key — rejected.
#[test]
fn public_key_layout_in_postcard() {
    let key = id();
    let bytes = postcard::to_stdvec(&key).unwrap();
    assert_eq!(
        bytes,
        key.as_bytes().to_vec(),
        "key = 32 bytes without a length"
    );
    let back: PeerId = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(back, key);
    let bad = [0x02u8; 32];
    assert!(
        postcard::from_bytes::<PeerId>(&bad).is_err(),
        "32 bytes that are not an ed25519 key must be rejected"
    );
}

#[test]
fn key_roundtrip() {
    for port in [1u16, 80, 65535] {
        assert_eq!(
            port_from_key(&port_key(Proto::Tcp, port)),
            Some((Proto::Tcp, port))
        );
    }
    assert_eq!(port_key(Proto::Tcp, 80), b"port/tcp/80/".to_vec());
}

#[test]
fn record_roundtrip() {
    for r in [
        rec("éøü", Audience::All),
        rec("x", Audience::Nodes(vec![id()])),
    ] {
        assert_eq!(decode_port_record(&encode_port_record(&r)), Some(r));
    }
}

#[test]
fn key_rejects_malformed() {
    let bad: [&[u8]; 17] = [
        b"port/tcp/080/",
        b"port/tcp/80",
        b"port/udp/080/",
        b"port/udp/80",
        b"port/udp/0/",
        b"port/udp/65536/",
        b"port/udp/+80/",
        b"port/udp/80/x/",
        b"port/tcp/0/",
        b"port/tcp/65536/",
        b"port/tcp/70000/",
        b"port/tcp/8a/",
        b"port/tcp/+80/",
        b"port/tcp/ 80/",
        b"port/tcp//",
        b"port/tcp/80/x/",
        b"port/tcp/\xff80/",
    ];
    for key in bad {
        assert_eq!(port_from_key(key), None, "key {key:?}");
    }
}

#[test]
fn record_layout_is_pinned() {
    let web = encode_port_record(&rec("éøü", Audience::All));
    assert_eq!(web, [0x01, 0x06, 0xC3, 0xA9, 0xC3, 0xB8, 0xC3, 0xBC, 0x00]);
    let mut want = vec![0x01, 0x00, 0x01, 0x01];
    want.extend_from_slice(id().as_bytes());
    assert_eq!(
        encode_port_record(&rec("", Audience::Nodes(vec![id()]))),
        want
    );
}

#[test]
fn decode_rejects() {
    let mut v2 = encode_port_record(&rec("a", Audience::All));
    v2[0] = 2;
    assert_eq!(decode_port_record(&v2), None);
    assert_eq!(decode_port_record(&[]), None);
    for n in 0u8..64 {
        let _ = decode_port_record(&[n; 3]);
        let bytes: Vec<u8> = (0..n).collect();
        let _ = decode_port_record(&bytes);
    }
    assert_eq!(decode_port_record(&[1, 0, 2]), None);
    assert_eq!(decode_port_record(&[1, 1, 0xFF, 0]), None);
    let long = rec(&"ü".repeat(65), Audience::All);
    assert_eq!(decode_port_record(&encode_port_record(&long)), None);
    let many = rec("", Audience::Nodes(vec![id(); 65]));
    assert_eq!(decode_port_record(&encode_port_record(&many)), None);
    let edge = rec(&"ü".repeat(64), Audience::Nodes(vec![id(); 64]));
    assert_eq!(decode_port_record(&encode_port_record(&edge)), Some(edge));
}

#[test]
fn pinned_v1_bytes_decode() {
    let web = [0x01, 0x06, 0xC3, 0xA9, 0xC3, 0xB8, 0xC3, 0xBC, 0x00];
    assert_eq!(decode_port_record(&web), Some(rec("éøü", Audience::All)));
    let mut nodes = vec![0x01, 0x00, 0x01, 0x01];
    nodes.extend_from_slice(id().as_bytes());
    assert_eq!(
        decode_port_record(&nodes),
        Some(rec("", Audience::Nodes(vec![id()])))
    );
}

#[test]
fn nodes_record_with_tail_decodes() {
    let mut bytes = vec![0x01, 0x00, 0x01, 0x01];
    bytes.extend_from_slice(id().as_bytes());
    bytes.extend_from_slice(&[9, 9]);
    assert_eq!(
        decode_port_record(&bytes),
        Some(rec("", Audience::Nodes(vec![id()])))
    );
}

#[test]
fn all_includes_anyone() {
    assert!(Audience::All.includes(&id()));
    assert!(Audience::All.includes(&other()));
}

#[test]
fn nodes_includes_listed_only() {
    let a = Audience::Nodes(vec![id(), id()]);
    assert!(a.includes(&id()));
    assert!(!a.includes(&other()));
}

#[test]
fn empty_nodes_includes_nobody() {
    assert!(!Audience::Nodes(vec![]).includes(&id()));
}

#[test]
fn name_for_listed_node_is_name() {
    let r = rec("web", Audience::Nodes(vec![id()]));
    assert_eq!(r.name_for(&id()), Some("web"));
}

#[test]
fn name_for_other_node_is_none() {
    let r = rec("web", Audience::Nodes(vec![id()]));
    assert_eq!(r.name_for(&other()), None);
}

#[test]
fn name_for_all_is_name() {
    assert_eq!(rec("web", Audience::All).name_for(&other()), Some("web"));
}

#[test]
fn decode_ignores_tail() {
    let r = rec("web", Audience::All);
    let mut bytes = encode_port_record(&r);
    bytes.extend_from_slice(&[9, 9, 9]);
    assert_eq!(decode_port_record(&bytes), Some(r));
}

#[test]
fn record_from_published() {
    let p = Published {
        port: 80,
        proto: Proto::Tcp,
        name: "web".into(),
        audience: Audience::All,
    };
    let r = PortRecord::from(&p);
    assert_eq!(r, rec("web", Audience::All));
    assert_eq!(r.v, 1);
}

fn golden_id_of(seed_hex: &str) -> PeerId {
    let seed: [u8; 32] = data_encoding::HEXLOWER
        .decode(seed_hex.as_bytes())
        .expect("seed — hex")
        .try_into()
        .expect("seed is 32 bytes");
    NodeSecret::from_bytes(&seed).peer_id()
}

#[test]
fn golden_audience_nodes_postcard_is_raw_32_bytes_per_key() {
    let key = golden_id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    // anchor RFC 8032 §7.1
    assert_eq!(
        key.to_string(),
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    );
    // variant number 01, list length 01, then 32 key bytes without their own length
    let mut want = vec![0x01u8, 0x01];
    want.extend_from_slice(&[
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07,
        0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07,
        0x51, 0x1a,
    ]);
    let audience = Audience::Nodes(vec![key]);
    let bytes = postcard::to_stdvec(&audience).expect("postcard");
    assert_eq!(bytes, want);
    assert_eq!(
        postcard::from_bytes::<Audience>(&want).expect("parse"),
        audience
    );

    assert_eq!(
        postcard::to_stdvec(&Audience::All).expect("postcard"),
        vec![0x00u8]
    );
    assert_eq!(
        postcard::from_bytes::<Audience>(&[0x00]).expect("parse"),
        Audience::All
    );
}

mod protocol_in_key {
    use super::*;

    #[test]
    fn prefixes_are_one_per_protocol() {
        assert_eq!(port_prefix(Proto::Tcp), "port/tcp/");
        assert_eq!(port_prefix(Proto::Udp), "port/udp/");
    }

    #[test]
    fn udp_key_lies_outside_the_tcp_prefix() {
        // given: the key of UDP port 53
        let key = port_key(Proto::Udp, 53);
        // then: the layout is `port/udp/53/`, and a scan of the TCP prefix would not reach it
        assert_eq!(key, b"port/udp/53/".to_vec());
        assert!(!key.starts_with(port_prefix(Proto::Tcp).as_bytes()));
        assert!(key.starts_with(port_prefix(Proto::Udp).as_bytes()));
    }

    #[test]
    fn same_port_has_different_keys_per_protocol() {
        assert_ne!(port_key(Proto::Tcp, 53), port_key(Proto::Udp, 53));
    }

    #[test]
    fn key_round_trips_for_each_protocol() {
        for proto in Proto::ALL {
            for port in [1u16, 53, 80, 65535] {
                assert_eq!(
                    port_from_key(&port_key(proto, port)),
                    Some((proto, port)),
                    "{proto:?} {port}"
                );
            }
        }
    }

    #[test]
    fn foreign_prefix_is_not_a_port() {
        let foreign: [&[u8]; 9] = [
            b"port/sctp/80/",
            b"port/TCP/80/",
            b"port/80/",
            b"port/tcp80/",
            b"port//80/",
            b"tcp/80/",
            b"port/",
            b"",
            b"ports/udp/80/",
        ];
        for key in foreign {
            assert_eq!(port_from_key(key), None, "key {key:?}");
        }
    }

    #[test]
    fn port_0_is_not_a_port_for_any_protocol() {
        for proto in Proto::ALL {
            assert_eq!(port_from_key(&port_key(proto, 0)), None, "{proto:?}");
        }
    }
}

mod record_value {
    use super::*;

    #[test]
    fn protocol_is_not_part_of_the_value() {
        // given: the same name and circle published over TCP and over UDP
        let tcp = Published {
            port: 53,
            proto: Proto::Tcp,
            name: "dns".into(),
            audience: Audience::All,
        };
        let udp = Published {
            proto: Proto::Udp,
            ..tcp.clone()
        };
        // then: the entry values are identical, the protocol lives in the key only
        assert_eq!(PortRecord::from(&udp), PortRecord::from(&tcp));
        assert_eq!(
            encode_port_record(&PortRecord::from(&udp)),
            encode_port_record(&PortRecord::from(&tcp))
        );
    }
}
