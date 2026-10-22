//! Topic membership by flooding over `SYNC` service connections (selected by `net.sync = "mesh"`).
//! `wire` — frames; `admit` — admission and handshake; `seen` — the duplicate cache; `links`, `peer` — peer links;
//! `topic` — the topic actor; `dial` — dialing a peer; `service` — accepting; `handle` — the topic handle; `backend` — the sync; `tuning` — timeouts.

pub(crate) mod admit;
mod backend;
mod dial;
mod handle;
mod links;
mod peer;
mod seen;
mod service;
mod topic;
mod tuning;
pub(crate) mod wire;

pub use backend::{sync_ctor, FloodSync, NAME};
pub use tuning::Tuning;

#[cfg(test)]
#[path = "backend_tests.rs"]
mod backend_tests;
#[cfg(test)]
#[path = "contract_tests.rs"]
mod contract_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "topic_down_tests.rs"]
mod topic_down_tests;
#[cfg(test)]
#[path = "topic_edge_tests.rs"]
mod topic_edge_tests;
#[cfg(test)]
#[path = "topic_tests.rs"]
mod topic_tests;
