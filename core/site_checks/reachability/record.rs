//! The check result record that goes into the shared document under the key `chk/<domain>/`.

use serde::{Deserialize, Serialize};

use super::verdict::CheckVerdict;

/// A `chk/<domain>/` record. It has no domain, addresses or node identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRecord {
    pub v: u8,
    pub verdict: CheckVerdict,
    pub dns_ms: u16,
    pub tcp_ms: u16,
    pub tls_ms: u16,
    pub http_ms: u16,
    pub status: u16,
    pub bytes: u32,
    pub stalled: bool,
    pub at: u64,
    pub reason: String,
}
