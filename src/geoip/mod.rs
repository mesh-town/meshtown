//! `geoip` — determining the node's country by its external IP and announcing it to peers.
//!
//! What it does: assembles the country code (`country`), the three answer sources (`sources`), the network
//! query of the sources (`fetch`) and the write/read in the network's shared document (`board`).
//! The check schedule and the `net_report` query are in `geoip::task`.
//! Links: `node_ctl::snapshot::collect` calls `read_countries`; `node::start_node` calls `task`.

mod board;
mod country;
mod fetch;
mod sources;
mod task;

pub use board::{announce_country, own_country_record, read_countries, COUNTRY_KEY};
pub use country::Country;
pub use fetch::lookup_country;
pub use sources::Source;
pub use task::{run_country_check, spawn_country_check};
