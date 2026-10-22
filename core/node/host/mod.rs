//! Process host: what the process owns once, however many nodes it starts and stops.
//! - `run` — the assembly point "services → page → body → page shutdown";
//! - `process` — the process services: the memory guard;
//! - `slots` — the map of network slots the page and the signal reach the nodes through;
//! - `signal` — the Ctrl-C/SIGTERM stop signal.

mod process;
mod run;
pub(crate) mod signal;
mod slots;

pub use crate::config::config_file::NetSlotId;
pub use process::ProcessServices;
pub use run::{Host, PageRole};
