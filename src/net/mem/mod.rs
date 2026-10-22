//! The substitute `mem` adapter: a network in process memory (hub, path, connector, discovery, sync; carrier).
//! There is no logic here, only declarations and re-exports.

mod carrier;
pub(crate) mod conn;
mod connector;
mod discovery;
pub(crate) mod frame;
mod hub;
mod path;
mod shared;
mod replica;
mod stream;
mod swarm;
mod sync;
mod sync_hub;
mod tasks;
mod transport;

#[cfg(test)]
mod contract_tests;

/// Kind of its own hint `AddrHint::Opaque`: "mem1".
pub const MEM_HINT_KIND: u64 = 0x6d65_6d31; // "mem1"

pub use carrier::MemCarrier;
pub use connector::MemConnector;
pub use discovery::MemDiscovery;
pub use hub::{process_net, MemNet};
pub use path::{MemPath, MEM_MAX_PACKET};
pub use replica::MemReplica;
pub use swarm::MemSwarm;
pub use sync::MemSync;
pub use sync_hub::{sync_ctor, MemSyncNet};
