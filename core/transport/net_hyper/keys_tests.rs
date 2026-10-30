//! Node key: one seed gives the same id in the mesh and in townjump.

use super::{key_pair, peer_of, relay_key_pair};
use crate::transport::net::error::NetError;
use crate::transport::net::id::NodeSecret;

fn assert_same_id(seed: [u8; 32]) {
    let secret = NodeSecret::from_bytes(&seed);
    let pair = key_pair(&secret);
    assert_eq!(&pair.public_key, secret.peer_id().as_bytes());
    assert_eq!(pair.secret_key[..32], seed);
}

#[test]
fn seed_gives_the_same_id_in_mesh_and_townjump() {
    assert_same_id([0; 32]);
    assert_same_id([7; 32]);
    assert_same_id(NodeSecret::generate().to_bytes());
}

#[test]
fn peer_of_accepts_a_real_key() {
    let id = NodeSecret::from_bytes(&[7; 32]).peer_id();
    assert_eq!(peer_of(id.as_bytes()).unwrap(), id);
}

#[test]
fn peer_of_rejects_a_key_off_the_curve() {
    let mut off = [0u8; 32];
    off[0] = 2; // y = 2: there is no such point on the ed25519 curve
    assert!(matches!(peer_of(&off), Err(NetError::Other(_))));
}

mod relay_key {
    use super::*;

    #[test]
    fn same_seed_gives_the_same_relay_key() {
        // given
        let seed = [7; 32];
        // when
        let first = relay_key_pair(&NodeSecret::from_bytes(&seed));
        let second = relay_key_pair(&NodeSecret::from_bytes(&seed));
        // then
        assert_eq!(first, second);
    }

    #[test]
    fn relay_key_differs_from_the_node_key() {
        // given
        let secret = NodeSecret::from_bytes(&[7; 32]);
        // when
        let relay = relay_key_pair(&secret);
        let node = key_pair(&secret);
        // then
        assert_ne!(relay.public_key, node.public_key);
    }

    #[test]
    fn other_seed_gives_another_relay_key() {
        // given
        let (a, b) = (
            NodeSecret::from_bytes(&[7; 32]),
            NodeSecret::from_bytes(&[8; 32]),
        );
        // when
        let (key_a, key_b) = (relay_key_pair(&a), relay_key_pair(&b));
        // then
        assert_ne!(key_a.public_key, key_b.public_key);
    }

    #[test]
    fn relay_key_is_a_valid_key_pair() {
        // given
        let secret = NodeSecret::from_bytes(&[7; 32]);
        // when
        let pair = relay_key_pair(&secret);
        // then: the public key is a point on the curve and ends the secret half
        assert!(
            peer_of(&pair.public_key).is_ok(),
            "relay key is off the curve"
        );
        assert_eq!(
            pair.secret_key[32..],
            pair.public_key,
            "the secret half does not end with the public key"
        );
    }
}
