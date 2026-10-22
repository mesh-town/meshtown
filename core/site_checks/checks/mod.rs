//! Check records in the shared document, the check schedule, and the node console.
//!
//! What it does: holds the check settings and the resolver, decides which domain is due for a check
//! (each node has its own constant offset, and the interval doubles for an unchanged result),
//! publishes the result under the key `chk/<domain>/`, reads the results of all nodes and prints them;
//! reads stdin and runs the commands `domains …`, `check <domain>`, `checks`.
//! What it doesn't do: doesn't combine node results into one verdict, doesn't build a route, doesn't keep
//! a check history and doesn't restrain an author who writes garbage.
//! Related: the domain list is `crate::proxy::domains`, measuring is `crate::site_checks::reachability`,
//! record values live in `iroh-blobs` blobs, output is `crate::node::output`.
//!
//! Files: `keys.rs` — the record key and the node snapshot (`CheckView`); `schedule.rs` — the schedule
//! (`Schedule`, "due" / "busy now"); `state.rs` — the `Checks` type and its constructor with live
//! handles; `store.rs` — publishing and reading the document's results; `describe.rs` — lines for
//! output; `run.rs` — the schedule loop; `console.rs` — console commands and starting it.

mod console;
mod describe;
mod keys;
mod run;
mod schedule;
mod state;
mod store;

/// The client string in check requests; defined in `reachability` (see there for why),
/// here so that `node.rs` takes it from the same module as the schedule.
pub use crate::site_checks::reachability::DEFAULT_USER_AGENT;

pub use console::{parse_check_line, spawn_console, CheckCommand};
pub use describe::{describe, describe_steps};
pub use keys::{check_key, domain_from_check_key, CheckView, CHECK_PREFIX};
pub use run::run_checks;
pub use schedule::{Schedule, FIRST_DELAY, FIRST_SPREAD, MAX_BACKOFF_EXPONENT};
pub use state::Checks;
