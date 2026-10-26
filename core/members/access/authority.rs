//! Records of power: what an admission, a grant, a revocation or a closing says.
//!
//! What it does: `Power` and `PowerIn` describe a live record of power; `power_of` tells the power
//! that a record kind carries.
//! What it doesn't do: no counting of records, no replica.

use crate::transport::net::PeerId;

use super::record::{AclKind, Via};

/// What a record of power says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    /// An admission by the given origin.
    Admit(Via),
    Grant,
    Revoke,
    Close,
}

/// A live record of power: `author` writes `power` about `subject` at `ts_micros`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerIn {
    pub author: PeerId,
    pub power: Power,
    pub subject: PeerId,
    pub ts_micros: u64,
}

/// The power a record kind carries; `None` for kinds that are not records of power.
pub(super) fn power_of(kind: AclKind, via: Via) -> Option<Power> {
    match kind {
        AclKind::Admit => Some(Power::Admit(via)),
        AclKind::Grant => Some(Power::Grant),
        AclKind::Revoke => Some(Power::Revoke),
        AclKind::Close => Some(Power::Close),
        _ => None,
    }
}

#[cfg(test)]
#[path = "authority_tests.rs"]
mod tests;
