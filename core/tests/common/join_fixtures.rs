//! Network keys for the `join` command tests. Included via `#[path]`, like the rest of `core/tests/common/*`.
//! Invites live in `join_invites.rs` and the fake page in `join_stub.rs`; a scenario includes only the leaf it needs.

use meshtown_core::members::network_key::managed_key_with_label;
use meshtown_core::transport::net::NodeSecret;

/// A network key with a label; `seed` tells the networks apart.
pub fn key_with_label(label: &str, seed: u8) -> String {
    let creator = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    managed_key_with_label(Some(label), &[seed; 32], &creator)
}
