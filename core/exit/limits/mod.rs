//! Limits of the exit side: a cap on live exit streams and a memory-based guard.
//!
//! What it does: counts live streams per process (`ExitLimits`), refuses new ones when the cap or
//! the memory ceiling is reached.
//! What it doesn't do: it doesn't touch open streams and doesn't read the machine memory size.
//!
//! Files: `gate` — the counter, the slot guard and the busy reasons; `rss` — the cached process
//! memory check and the ceiling formula.

mod gate;
mod line_rate;
mod rss;

pub(crate) use gate::Busy;
pub use gate::ExitLimits;
#[cfg(test)]
pub(crate) use gate::StreamSlot;
pub(crate) use line_rate::LineRate;
pub(crate) use rss::rss_ceiling_kib;
