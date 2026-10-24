//! Membership list ("who may be a neighbour"): record format, vote tally, verdict view, boot,
//! watcher and member actions.
//!
//! What it does: declares the modules of the list.
//! What it doesn't do: holds no logic.

pub mod actions;
mod admission;
pub mod authority;
pub mod boot;
mod boot_plan;
mod deny;
mod forget;
mod reasons;
pub mod record;
mod record_input;
mod record_io;
pub mod report;
mod report_forget;
pub mod runtime;
mod subjects;
pub mod tally;
pub mod view;
pub mod watch;
