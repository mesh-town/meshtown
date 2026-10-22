//! Process host: what the process owns once, however many nodes it starts and stops.
//! - `run` — the assembly point "services → page → body → page shutdown";
//! - `process` — the process services: the memory guard;
//! - `signal` — the Ctrl-C/SIGTERM stop signal.

mod process;
mod run;
pub(crate) mod signal;

pub use process::ProcessServices;
pub use run::{Host, PageRole};
