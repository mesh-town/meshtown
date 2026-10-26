//! Golden test of the `mesh-invite2-` invite string (format 4): the string is a literal recomputed outside the code
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
        root: NodeSecret::from_bytes(&[7u8; 32]).peer_id(),
    }
}

#[test]
fn golden_invite_is_version_4() {
    const INVITE: &str =
        "mesh-invite2-atlk27cqadd4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pr6hy7d4pahxytkqmaoxlkmadavrbk35ks762pewibz2b3qxf462uyrsllycdjupob2rdiaacad7aaaadia7qd34jvigahveu3dd4kofecv66vihwezoyx4zkr3wv27l464siipou2iui3jcy";
    let id_me = id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let h = head();
    let addr: SocketAddr = "127.0.0.1:4000".parse().expect("address");
    let me = from_endpoint_addr(
        &crate::transport::net::endpoint_shim::EndpointAddr::new(id_me).with_ip_addr(addr),
    );

    let s = build_keyed(&h, &me, &[], &PrimaryIps::NONE).expect("build");
    assert_eq!(s, INVITE);

    let body = s.strip_prefix("mesh-invite2-").expect("prefix");
    let bytes = data_encoding::BASE32_NOPAD
        .decode(body.to_ascii_uppercase().as_bytes())
        .expect("base32 body");
    assert_eq!(bytes[0], 4);
    let parsed = parse(&s).expect("parse");
    assert_eq!(parsed.head.root, h.root);
    assert_eq!(parsed.head.network_id, h.network_id);
    assert_eq!(parsed.head.secret, h.secret);
    assert_eq!(parsed.peers[0].id, id_me);
}
