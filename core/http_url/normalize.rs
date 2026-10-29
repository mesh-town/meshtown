//! Normal form of an http/https address, the way the `url` crate prints it for practical inputs.

use super::host::{host_to_ascii, split_host_port};
use super::UrlError;

/// Lowercase scheme and host (IDN as punycode), default port dropped, empty path as `/`.
/// Refused: other schemes, login, characters that would need percent-encoding, backslashes,
/// dot segments, non-canonical IPv4.
pub fn normalize(input: &str) -> Result<String, UrlError> {
    let s = input.trim_matches(|c: char| c <= ' ');
    if s.contains(['\t', '\n', '\r', '\\']) {
        return Err(UrlError("forbidden character"));
    }
    let (scheme, rest) = s.split_once("://").ok_or(UrlError("no scheme"))?;
    let scheme = scheme.to_ascii_lowercase();
    let default = default_port(&scheme)?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return Err(UrlError("login is not supported"));
    }
    let (raw, port) = split_host_port(authority)?;
    let host = host_to_ascii(raw)?;
    check_tail(tail)?;
    let port = port.filter(|p| *p != default).map_or(String::new(), |p| format!(":{p}"));
    Ok(format!("{scheme}://{host}{port}{}", with_root(tail)))
}

fn default_port(scheme: &str) -> Result<u16, UrlError> {
    match scheme {
        "http" => Ok(80),
        "https" => Ok(443),
        _ => Err(UrlError("only http and https are supported")),
    }
}

fn with_root(tail: &str) -> String {
    match tail.chars().next() {
        Some('/') => tail.to_string(),
        _ => format!("/{tail}"),
    }
}

/// Path, query and fragment must be kept byte for byte, so nothing that `url` would escape.
fn check_tail(tail: &str) -> Result<(), UrlError> {
    let clean = tail.chars().all(|c| c.is_ascii_graphic() && !"\"<>^`{}".contains(c));
    let path = &tail[..tail.find(['?', '#']).unwrap_or(tail.len())];
    let query = tail.strip_prefix(path).unwrap_or("");
    let quoted = query.split('#').next().is_some_and(|q| q.contains('\''));
    match (clean && !quoted, path.split('/').any(is_dot_segment)) {
        (true, false) => Ok(()),
        _ => Err(UrlError("address needs percent-encoding")),
    }
}

fn is_dot_segment(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    matches!(lower.as_str(), "." | ".." | "%2e" | ".%2e" | "%2e." | "%2e%2e")
}

#[cfg(test)]
#[path = "normalize_tests.rs"]
mod tests;
