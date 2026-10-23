//! Node assembly: ties all modules into a working process.
//!
//! What it does: brings up the transports (`hyper`; `mem` in tests), the sync (swarm and domain list replica), availability
//! checks, the network stack with discovery (`invite`, `cache`, `hyperdht`) and the node's main loop (member discovery,
//! handshakes, invites, periodic status).
//! What it doesn't do: doesn't parse command-line arguments — that is `main.rs`.
//! Links: uses all the other library modules; `main.rs` calls `run_node_with_ctl`/`run_node_awaiting_key`.
//!
//! Files:
//! - `identity` — the node identity state (`IdentityState`, `TemporaryReason`).
//! - `config` — node settings for assembly (`NodeConfig`).
//! - `running` — the running node and its live handles (`RunningNode`).
//! - `start` — assembling and starting the node (`start_node`, `bind_ephemeral`), split into start phases.
//! - `event_loop` — the node's main loop: discovered addresses, member events, dial failures,
//!   retry deadlines and periodic tasks.
//! - `guard` — the process memory guard: a separate OS thread measures RSS once a second, prints `[guard]` and exits before swap.
//! - `alloc` — the jemalloc allocator with a heap profile (`heap-prof` feature, Linux only) for the memory guard.
//! - `dial` — dialing a discovered member and the handshake as the initiator (`PeerDialer`).
//! - `redial` — the redial state of a not-yet-connected member, with no network (`Redial`).
//! - `lifecycle` — the "start → run → stop or restart" loops
//!   (`run`, `run_node_with_ctl`, `run_node_awaiting_key`).

#[cfg(feature = "net-hyper")]
pub(crate) mod fd_limit;
#[cfg(feature = "net-hyper")]
pub mod host;
#[cfg(any(test, all(target_arch = "arm", target_env = "musl")))]
pub mod libm_shim;
#[cfg(feature = "net-hyper")]
pub mod node_ctl;
#[cfg(feature = "net-hyper")]
pub mod node_name;
#[cfg(feature = "net-hyper")]
pub mod node_state;
#[cfg(feature = "net-hyper")]
pub mod output;
#[cfg(feature = "net-hyper")]
pub mod program_version;
pub mod tcp_tune;
#[cfg(feature = "net-hyper")]
pub mod telemetry;

#[cfg(feature = "net-hyper")]
mod alloc;
#[cfg(feature = "net-hyper")]
mod config;
#[cfg(feature = "net-hyper")]
mod dial;
#[cfg(feature = "net-hyper")]
mod discovery_sources;
#[cfg(feature = "net-hyper")]
mod doc_changes;
#[cfg(feature = "net-hyper")]
mod event_loop;
#[cfg(feature = "net-hyper")]
mod guard;
#[cfg(feature = "net-hyper")]
mod guard_sources;
#[cfg(feature = "net-hyper")]
mod identity;
#[cfg(feature = "net-hyper")]
mod lifecycle;
#[cfg(feature = "net-hyper")]
pub(crate) mod net_names;
#[cfg(feature = "net-hyper")]
mod peer_path;
#[cfg(feature = "net-hyper")]
mod redial;
#[cfg(feature = "net-hyper")]
mod rejoin;
#[cfg(feature = "net-hyper")]
mod running;
#[cfg(feature = "net-hyper")]
#[cfg(feature = "net-hyper")]
mod start;

#[cfg(feature = "net-hyper")]
pub use config::NodeConfig;
#[cfg(feature = "net-hyper")]
pub use doc_changes::DocChanges;
#[cfg(feature = "net-hyper")]
pub use identity::{IdentityState, TemporaryReason};
#[cfg(feature = "net-hyper")]
pub use lifecycle::{run, run_network, run_node_awaiting_key, run_node_with_ctl};
#[cfg(feature = "net-hyper")]
pub use running::RunningNode;
#[cfg(feature = "net-hyper")]
pub use start::{bind_ephemeral, start_node};
