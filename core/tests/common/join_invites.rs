//! Invites for the `join` command tests: parsable, issued by a node that is not there. Included via `#[path]`.

use meshtown_core::invite::{build_keyed, InviteHead, InviteSecret};
use meshtown_core::members::network_key::parse_user_input;
use meshtown_core::transport::addr_order::PrimaryIps;
use meshtown_core::transport::net::{NodeSecret, PeerAddr};
use meshtown_core::transport::net_mainline::unix_now;

/// A fresh invite to the network of `key`, issued by a node that is not there: it parses and passes the age
/// check, and no member ever answers it.
pub fn unanswered_invite(key: &str) -> String {
    invite_made_at(key, unix_now())
}

/// An invite to the network of `key` made at `made_unix` (seconds); an old time makes it expired.
pub fn invite_made_at(key: &str, made_unix: u64) -> String {
    let issuer = NodeSecret::from_bytes(&[2u8; 32]).peer_id();
    let parsed = parse_user_input(key).expect("a key");
    let head = InviteHead {
        secret: InviteSecret::generate().expect("secret"),
        network_id: parsed.key.subkeys().network_id,
        label: parsed.label,
        made_unix,
        root: NodeSecret::from_bytes(&[1u8; 32]).peer_id(),
    };
    build_keyed(&head, &PeerAddr::new(issuer), &[], &PrimaryIps::NONE).expect("invite text")
}
