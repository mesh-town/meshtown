//! Transport: the network layer under the node.
//!
//! - `net` - the network interface of the node (traits and types without foreign network crates).
//! - `net_hyper` - the implementation of `net` over the HyperDHT crates.
//! - `net_hyperdht` - the HyperDHT client.
//! - `net_mainline` - the roster record format and the clock.
//! - `peers_cache` - the cache of members' last working addresses on disk.
//! - `addr_order` - the order in which addresses are tried.
//! - `transfer_probe` - the volume probe over a connection.

pub mod addr_order;
pub mod net;
#[cfg(feature = "net-hyper")]
pub mod net_hyper;
#[cfg(feature = "net-hyper")]
pub mod net_hyperdht;
#[cfg(feature = "net-hyper")]
pub mod net_mainline;
#[cfg(feature = "net-hyper")]
pub mod peers_cache;
#[cfg(feature = "net-hyper")]
pub mod transfer_probe;
