//! The `tray` role of the subcommand: runtime and graphics checks, config reading, its own polling loop
//! for the node, and the switch to the tray loop. Files: `run.rs` — arguments and startup (`TrayArgs`, `run_tray`), the link
//! to the page, reading `web-token`, child processes; `poll_types.rs` — timeouts and `PollEnv` between
//! startup and the poller; `api.rs` — talking to the node over HTTP and the start policy; `views.rs` —
//! ready and under-construction tray views; `poller.rs` — the poller's main loop (event selection);
//! `poller_phase.rs` — the phase and transitions on the node's reply; `poller_stop.rs` — the "Stop" and
//! "Quit" procedures; `poller_exit.rs` — editing the list of selected exits on a command from the submenu and the "Allow opening sites
//! via this computer" checkbox (`exit.enabled`);
//! `poller_invite.rs` — "Invite a member": the node's invite to the clipboard
//! and the return of the item's label via `PollTiming.note_visible_for`.
//! `first_run.rs` — opens the node page once per tray process when the node has no network yet (the setup wizard).
//! `start_fail.rs` — the reason the node refused, for the menu (proxy port, node log).
//! Links: `run_tray()` is the only call from outside (`app::mod`/`main.rs`), it passes the poller
//! `poller::poll_node` and gets the exit code back from it; the poller itself knows nothing of `crate::gui::tray`.

mod api;
mod config_doc;
mod first_run;
mod poll_types;
mod poller;
mod poller_exit;
mod poller_invite;
mod poller_phase;
mod poller_stop;
mod run;
mod start_fail;
mod views;

pub use run::{run_tray, TrayArgs};
