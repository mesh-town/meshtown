//! The `hyperdht` discovery source: a key roster in a DHT record (no logic).
//!
//! `store` is a narrow record store trait, `keys` — record and encryption keys,
//! `pace` — round pacing, `tally` — the log; `mem` — an in-memory store for tests.

mod keys;
mod pace;
mod roster;
mod source;
mod store;
mod tally;

#[cfg(test)]
mod mem;

pub use keys::RosterKeys;
pub use pace::Pace;
pub use store::{Record, RecordKey, RecordStore};
pub use source::{node_source, DhtRoundOutcome, HyperDhtDiscovery, RoundOutcomeFn};
