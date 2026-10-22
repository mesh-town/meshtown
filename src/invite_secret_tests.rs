//! Tests of `invite.rs`: invite secret.

use super::*;

#[test]
fn generated_secrets_differ_and_are_not_zero() {
    let a = InviteSecret::generate().unwrap();
    let b = InviteSecret::generate().unwrap();
    assert_ne!(a, b, "two calls gave one secret");
    assert_ne!(a.as_bytes(), &[0u8; 32], "all-zero secret");
}

#[test]
fn bytes_round_trip() {
    let bytes: [u8; 32] = core::array::from_fn(|i| 0x40 + i as u8);
    let secret = InviteSecret::from_bytes(bytes);
    assert_eq!(secret.as_bytes(), &bytes);
    assert_eq!(secret, InviteSecret::from_bytes(bytes));
}
