//! Tests of the two texts of an issued invite: the full string and the shorter one for the QR code.

use super::*;
use crate::invite::{parse, InviteSecret};
use crate::transport::net::NodeSecret;

fn addr(n: u8) -> PeerAddr {
    PeerAddr::new(NodeSecret::from_bytes(&[n; 32]).peer_id())
}

#[test]
fn qr_invite_keeps_first_three_peers() {
    // given: a list of five members
    let head = InviteHead {
        secret: InviteSecret::from_bytes([0x11; 32]),
        network_id: [0xaa, 0xbb, 0xcc, 0xdd],
        label: None,
        made_unix: 1_760_000_000,
        root: addr(7).id,
    };
    let list: Vec<PeerAddr> = (1..=5).map(addr).collect();
    // when
    let (text, qr_text) = invite_texts(&head, &list, &PrimaryIps::NONE).unwrap();
    let full = parse(&text).unwrap();
    let short = parse(&qr_text).unwrap();
    // then: the first three members in the same order, the same head
    assert_eq!(full.peers.len(), 5);
    assert_eq!(
        format!("{:?}", short.peers),
        format!("{:?}", &full.peers[..3])
    );
    assert_eq!(format!("{:?}", short.head), format!("{:?}", full.head));
    assert!(qr_text.len() < text.len());
}
