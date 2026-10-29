//! Canonical text of IP literals in a host, in the compression the WHATWG URL standard uses.

use super::UrlError;
use std::net::Ipv6Addr;

/// The inside of `[...]`: lowercase hex, the longest run (two or more) of zero groups as `::`,
/// no embedded IPv4.
pub(super) fn ipv6(inner: &str) -> Result<String, UrlError> {
    let addr: Ipv6Addr = inner.parse().map_err(|_| UrlError("bad IPv6 address"))?;
    let groups = addr.segments();
    let (start, len) = longest_zero_run(&groups);
    Ok(format!("[{}]", compressed(&groups, start, len)))
}

fn compressed(groups: &[u16; 8], start: usize, len: usize) -> String {
    if len < 2 {
        return join(groups);
    }
    format!("{}::{}", join(&groups[..start]), join(&groups[start + len..]))
}

fn join(groups: &[u16]) -> String {
    let parts: Vec<String> = groups.iter().map(|g| format!("{g:x}")).collect();
    parts.join(":")
}

fn longest_zero_run(groups: &[u16; 8]) -> (usize, usize) {
    let (mut best, mut cur) = ((0, 0), (0, 0));
    for (i, g) in groups.iter().enumerate() {
        cur = if *g == 0 { (cur.0, cur.1 + 1) } else { (i + 1, 0) };
        if cur.1 > best.1 {
            best = cur;
        }
    }
    best
}

/// True when the host ends in a number the URL standard would read as an IPv4 part.
pub(super) fn ends_in_number(host: &str) -> bool {
    let trimmed = host.strip_suffix('.').unwrap_or(host);
    let last = trimmed.rsplit('.').next().unwrap_or("");
    let hex = last
        .strip_prefix("0x")
        .or_else(|| last.strip_prefix("0X"))
        .is_some_and(|rest| rest.bytes().all(|b| b.is_ascii_hexdigit()));
    hex || (!last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()))
}

/// Dotted-decimal only: four parts, each 0..=255 without leading zeros. A trailing dot is dropped.
pub(super) fn ipv4(host: &str) -> Result<String, UrlError> {
    let trimmed = host.strip_suffix('.').unwrap_or(host);
    let parts: Vec<&str> = trimmed.split('.').collect();
    let canonical = parts.len() == 4 && parts.iter().all(|p| canonical_octet(p));
    canonical
        .then(|| trimmed.to_string())
        .ok_or(UrlError("non-canonical IPv4 address"))
}

fn canonical_octet(part: &str) -> bool {
    let digits = !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    let plain = part == "0" || !part.starts_with('0');
    digits && plain && part.parse::<u8>().is_ok()
}
