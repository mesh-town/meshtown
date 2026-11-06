//! The whole `config` document (`Settings` = process and network blocks), `ListenAccess`.
//! Not here: JSON parsing (`parse.rs`/`parse_fields.rs`) and writing (`render.rs`).

use super::net_settings::NetSettings;
use super::process_settings::ProcessSettings;

/// Who may connect to a listening port — the node page (`web.access`) or the proxy (`socks.access`): this machine only, or the local network too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenAccess {
    Localhost,
    Localnet,
}

impl ListenAccess {
    /// Returns the value in the file: `localhost` | `localnet`.
    pub fn as_str(self) -> &'static str {
        match self {
            ListenAccess::Localhost => "localhost",
            ListenAccess::Localnet => "localnet",
        }
    }

    /// The inverse of `as_str`; an unknown string gives `None`.
    pub fn parse(s: &str) -> Option<ListenAccess> {
        match s {
            "localhost" => Some(ListenAccess::Localhost),
            "localnet" => Some(ListenAccess::Localnet),
            _ => None,
        }
    }
}

/// The whole `config` document: the process part and the network part, every field filled. Defaults live in
/// the blocks' `Default`. `Debug` is hand-written and omits the network key: `Settings` lives in `ParsedConfigFile` and `RunConfig`,
/// and a failed `assert_eq!` or a future `{:?}` in the log would otherwise print the whole network key.
#[derive(Clone, PartialEq, Default)]
pub struct Settings {
    pub process: ProcessSettings, // SOCKS, page, log, memory guard — the node reads only SOCKS
    pub net: NetSettings,         // everything the running node reads
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        super::fields::debug_settings(self, f)
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "settings_split_tests.rs"]
mod split_tests;
