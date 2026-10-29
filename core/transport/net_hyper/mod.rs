//! The `hyper` transport: an adapter to townjump. Only the module list (the content is per file).
//! The modules have no consumers yet: `pub` keeps `dead_code` clean until the transport is built.

pub mod accept;
pub mod conn;
mod dht_table;
pub mod gate;
pub mod keys;
pub mod link;
mod netchange;
mod records;
mod relay;
pub mod table;
pub mod transport;
pub mod wire;

pub use relay::{RelayAdmit, RelayCandidate, RelaySource};
pub use transport::{HyperOpts, HyperTransport, KIND};

#[cfg(test)]
mod admit_tests;
#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod probe_tests;
#[cfg(test)]
mod relay_contract_tests;
#[cfg(test)]
pub(crate) mod test_cluster;
#[cfg(test)]
pub(crate) mod test_direct;
#[cfg(test)]
pub(crate) mod test_relay;
