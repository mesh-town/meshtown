//! Counting of the exit's admission decisions and the "overloaded" state derived from them.
//!
//! What it does: keeps a ring of 15-second buckets with admitted and refused streams (per process
//! and per member) and answers whether the exit is overloaded and why.
//! What it doesn't do: it doesn't change admission; the network document is written only by `announce`.
//!
//! Files: `board` — the record's key, format and reading; `announce` — keeps the record in line with the meter;
//! `buckets` — the ring and the time arithmetic; `counts` — what a bucket holds;
//! `verdict` — the pure state function and its constants; `meter` — the counter the gate writes to.

mod announce;
// The reader of others' records is used by the state snapshot in a later step.
#[allow(dead_code)]
mod board;
mod buckets;
mod client;
mod counts;
mod meter;
mod verdict;

pub use announce::run_exit_load_announce;
// Read by the state snapshot in a later step.
#[allow(unused_imports)]
pub(crate) use board::{read_exit_loads, Cause, ExitLoad};
pub(crate) use client::{Answer, ExitAnswers};
pub(crate) use counts::Outcome;
pub(crate) use meter::{LoadSnapshot, Meter};
pub(crate) use verdict::Reason;
