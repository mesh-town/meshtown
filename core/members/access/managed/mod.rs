//! The tally of an administered network: roles, exclusions and the closing of the network.
//!
//! What it does: declares the modules of the managed tally.
//! What it doesn't do: holds no logic.

mod closure;
mod events;
mod roles;

pub(in crate::members::access) use closure::tally;

#[cfg(test)]
#[path = "support_tests.rs"]
mod support;
