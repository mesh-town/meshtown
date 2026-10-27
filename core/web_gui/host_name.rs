//! Parsing of the `Host` value `<label>.meshtown[:<port>]` that a page opened through the mesh carries.

use std::net::IpAddr;

/// The only domain suffix accepted besides the socket literal.
const SUFFIX: &str = ".meshtown";

/// Whether `value` is `<label>.meshtown` or `<label>.meshtown:<port>`: one label of `[a-z0-9-]`
/// (1–63 characters, letter case ignored), a decimal port in 1–65535.
pub(super) fn is_mesh_host_name(value: &str) -> bool {
    let (name, port) = split_port(value);
    port.is_none_or(is_port) && strip_suffix_ci(name).is_some_and(is_label)
}

/// Whether the request came from this very machine (`127.0.0.0/8`, `::1`).
pub(super) fn is_loopback_client(ip: IpAddr) -> bool {
    ip.to_canonical().is_loopback()
}

fn split_port(value: &str) -> (&str, Option<&str>) {
    match value.split_once(':') {
        Some((name, port)) => (name, Some(port)),
        None => (value, None),
    }
}

fn strip_suffix_ci(name: &str) -> Option<&str> {
    let cut = name.len().checked_sub(SUFFIX.len())?;
    let tail = name.get(cut..)?;
    tail.eq_ignore_ascii_case(SUFFIX).then(|| &name[..cut])
}

fn is_port(text: &str) -> bool {
    let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    digits && text.len() <= 5 && text.parse::<u32>().is_ok_and(|p| (1..=65535).contains(&p))
}

fn is_label(text: &str) -> bool {
    let sized = (1..=63).contains(&text.len());
    sized && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[cfg(test)]
#[path = "host_name_tests.rs"]
mod tests;
