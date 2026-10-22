//! The check record key in the shared document (`chk/<domain>/`) and a snapshot of one node for one
//! domain (`CheckView`), which `store.rs` assembles from here.

use crate::domains;
use crate::net::PeerId;
use crate::reachability::CheckRecord;

/// Key prefix of check results in the document.
pub const CHECK_PREFIX: &str = "chk/";

/// The check record key; the trailing `/` is mandatory for the same reason as in `list/`.
pub fn check_key(domain: &str) -> Vec<u8> {
    format!("{CHECK_PREFIX}{domain}/").into_bytes()
}

/// Extracts the domain from a `chk/<domain>/` key; foreign and non-normalized keys give `None`.
pub fn domain_from_check_key(key: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(key).ok()?;
    let rest = text.strip_prefix(CHECK_PREFIX)?;
    let domain = rest.strip_suffix('/')?;
    let normal = domains::normalize_domain(domain).ok()?;
    if normal == domain {
        Some(normal)
    } else {
        None
    }
}

/// The result of one node for one domain.
#[derive(Debug, Clone)]
pub struct CheckView {
    pub domain: String,
    pub author: PeerId,
    pub entry_ts_micros: u64,
    /// `None` if the record content has not been downloaded yet or the format is foreign.
    pub record: Option<CheckRecord>,
}

/// Keeps the records of domains from the node's list, preserving order: `chk/` in the shared document
/// carries the domains of all nodes, and only its own are shown.
pub(crate) fn only_listed(views: Vec<CheckView>, list: &[String]) -> Vec<CheckView> {
    views
        .into_iter()
        .filter(|v| list.contains(&v.domain))
        .collect()
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
