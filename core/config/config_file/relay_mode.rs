//! `RelayMode`: whether the node may use (and serve as) a relay inside its own network (`net.relay`).

/// How the node treats relaying inside its network. More values may be added later.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RelayMode {
    /// Never relay, never ask a relay.
    Off,
    /// Use members of the same network as relays, and serve them when reachable.
    #[default]
    Network,
}

impl RelayMode {
    /// Returns the value in the file: `off` | `network`.
    pub fn as_str(self) -> &'static str {
        match self {
            RelayMode::Off => "off",
            RelayMode::Network => "network",
        }
    }

    /// The inverse of `as_str`; an unknown string gives `None`.
    pub fn parse(s: &str) -> Option<RelayMode> {
        match s {
            "off" => Some(RelayMode::Off),
            "network" => Some(RelayMode::Network),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "relay_mode_tests.rs"]
mod tests;
