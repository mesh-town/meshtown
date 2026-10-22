//! Settings and local storage next to the config.
//!
//! - `config_file` - the settings file on disk: lookup, reading, merging with flags, writing.
//! - `docs_store` - the path of the domain list storage directory next to the config.

#[cfg(feature = "net-hyper")]
pub mod config_file;
#[cfg(feature = "net-hyper")]
pub mod docs_store;
