//! Key texts of the host and control tests: a Managed network key made of a seed.

use crate::members::network_key::managed_key_with_label;
use crate::transport::net::contract::pid;

/// The text of a Managed network key whose base and creator derive from `seed`; one seed, one network.
pub(crate) fn managed_text(seed: u8) -> String {
    managed_key_with_label(None, &[seed; 32], &pid(seed))
}

/// The parsed form of `managed_text(seed)`, the argument of `Relaunch::node_config`.
pub(crate) fn key_input(seed: u8) -> crate::members::network_key::NetworkKeyInput {
    crate::members::network_key::parse_user_input(&managed_text(seed)).expect("a managed key")
}
