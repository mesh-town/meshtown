//! The domain check result class and its short name for output.

use serde::{Deserialize, Serialize};

/// The result class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckVerdict {
    Ok,
    DnsMismatch,
    NetworkBlocked,
    SiteGeoblock,
    Antibot,
    Down,
}

impl CheckVerdict {
    /// The short name of the class for output.
    pub fn text(self) -> &'static str {
        match self {
            CheckVerdict::Ok => "opens",
            CheckVerdict::DnsMismatch => "blocked: address spoofed",
            CheckVerdict::NetworkBlocked => "blocked by ISP",
            CheckVerdict::SiteGeoblock => "blocked for this country",
            CheckVerdict::Antibot => "blocked as a bot",
            CheckVerdict::Down => "site is down",
        }
    }
}
