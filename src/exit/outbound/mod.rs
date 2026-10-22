//! Where and how the exit connects outward: the address guard, address choice, refusals and dialing.
//!
//! Files: `address_guard` — filtering of one destination address; `refusal` — a refusal: reply code plus the log
//! line; `target` — choosing the address from an IP literal or a resolver answer; `dial` — name or literal to an
//! open TCP stream.

mod address_guard;
mod dial;
mod refusal;
mod target;

pub(in crate::exit) use address_guard::{first_allowed, is_forbidden, is_own_address};
pub(in crate::exit) use dial::{open_tcp, resolve_address};
pub(in crate::exit) use refusal::Refusal;
