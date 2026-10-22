//! Which network stack names are built into this binary (Cargo features of the node layer).
//!
//! What it does: returns the lists of allowed names of transport, peer discovery and replica sync, and
//! the network choice default without the names that aren't built.
//! What it doesn't do: doesn't build the stack and doesn't read the config — that is `start/network.rs` and `config_file`.
//! Links: `config_file::parse_net` checks the `net` section of the settings file against these lists.

use crate::transport::net::NetChoice;

/// Transports (how packets travel between nodes); `mem` isn't in the list — only `NodeConfig.test_net` chooses it.
pub(crate) fn transports() -> Vec<&'static str> {
    vec!["hyper"]
}

/// Peer discovery sources.
pub(crate) fn discovery() -> Vec<&'static str> {
    vec!["invite", "cache", "hyperdht"]
}

/// Replica sync (the network's shared state): in memory or its own (`mesh`: membership by flooding).
pub(crate) fn sync() -> Vec<&'static str> {
    vec!["mem", "mesh"]
}

/// The `NetChoice` default without the names that aren't in the lists above.
pub(crate) fn default_choice() -> NetChoice {
    let mut choice = NetChoice::default();
    let have = discovery();
    choice.discovery.retain(|n| have.contains(&n.as_str()));
    choice
}

#[cfg(test)]
#[path = "net_names_tests.rs"]
mod tests;
