//! Node key: one seed gives the same id in the mesh and in townjump.

use super::{key_pair, peer_of};
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
