//! The node's own replica of entries "(author, key) → value, time, signature". Only the module wiring is here.

mod backend;
pub(crate) mod entry;
pub(crate) mod file;
mod host;
pub(crate) mod ids;
pub(crate) mod import;
#[allow(clippy::module_inception)] // the file by design: `replica::MeshReplica`
mod replica;
mod replica_session;
mod replica_swarm;
mod replica_sync;
mod saver;
mod service;
pub(crate) mod session;
pub(crate) mod state;
pub(crate) mod wire;

pub use backend::{sync_ctor, MeshSync, NAME};

#[cfg(test)]
#[path = "backend_tests.rs"]
mod backend_tests;
#[cfg(test)]
#[path = "contract_tests.rs"]
mod contract_tests;
#[cfg(test)]
#[path = "replica_net_tests.rs"]
mod replica_net_tests;
#[cfg(test)]
#[path = "replica_tests.rs"]
mod replica_tests;
#[cfg(test)]
pub(crate) mod test_support;
