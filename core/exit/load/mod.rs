//! Counting of the exit's admission decisions and the "overloaded" state derived from them.
//!
//! What it does: keeps a ring of 15-second buckets with admitted and refused streams (per process
//! and per member) and answers whether the exit is overloaded and why.
//! What it doesn't do: it doesn't write anything to the network document and doesn't change admission.
//!
//! Files: `buckets` — the ring and the time arithmetic; `counts` — what a bucket holds;
//! `verdict` — the pure state function and its constants; `meter` — the counter the gate writes to.

// The meter is written by the gate now; the readers (announcer, state snapshot) come in later steps.
#![allow(dead_code)]

mod buckets;
mod counts;
mod meter;
mod verdict;

pub(crate) use counts::Outcome;
pub(crate) use meter::{LoadSnapshot, Meter};
pub(crate) use verdict::Reason;
