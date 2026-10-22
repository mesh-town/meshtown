//! The network member record (roster format) and the clock. The mainline DHT network part has been removed;
//! the directory name stayed the same (renaming is a separate task).

pub mod record;

mod clock;

pub use clock::unix_now;
pub use record::{
    entry_from_addr, entry_to_addr, merge, open, seal, Roster, RosterEntry, ENTRY_TTL_SECS,
};
