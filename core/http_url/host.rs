//! The host part of an address: authority splitting and conversion of a host to its ASCII form
//! (lowercase, IDN labels as punycode, canonical IP literals).

use super::ip;
use super::punycode::encode_label;
use super::UrlError;
use icu_normalizer::ComposingNormalizerBorrowed;

/// The host of an address with any scheme (or none): scheme, login, port, path, `?` and `#`
/// are dropped. IPv6 comes back in brackets.
pub fn host(input: &str) -> Result<String, UrlError> {
    let rest = input.split_once("://").map_or(Ok(input), |(scheme, rest)| {
        valid_scheme(scheme).then_some(rest).ok_or(UrlError("bad scheme"))
    })?;
    let authority = &rest[..rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len())];
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, hp)| hp);
    let (raw, _port) = split_host_port(host_port)?;
    host_to_ascii(raw)
}

pub(super) fn valid_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    let head = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    head && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Splits `host[:port]`; an IPv6 literal keeps its brackets. An empty port is no port.
pub(super) fn split_host_port(host_port: &str) -> Result<(&str, Option<u16>), UrlError> {
    let (raw, tail) = if host_port.starts_with('[') {
        let end = host_port.find(']').ok_or(UrlError("unclosed bracket"))?;
        host_port.split_at(end + 1)
    } else {
        host_port.find(':').map_or((host_port, ""), |i| host_port.split_at(i))
    };
    Ok((raw, parse_port(tail)?))
}

fn parse_port(tail: &str) -> Result<Option<u16>, UrlError> {
    let Some(digits) = tail.strip_prefix(':') else {
        return if tail.is_empty() { Ok(None) } else { Err(UrlError("bad port")) };
    };
    let numeric = digits.bytes().all(|b| b.is_ascii_digit());
    match (numeric, digits.is_empty()) {
        (false, _) => Err(UrlError("bad port")),
        (true, true) => Ok(None),
        (true, false) => digits.parse().map(Some).map_err(|_| UrlError("bad port")),
    }
}

/// ASCII form of a host as written between the authority delimiters.
pub(crate) fn host_to_ascii(raw: &str) -> Result<String, UrlError> {
    let Some(inner) = raw.strip_prefix('[') else {
        return domain(raw);
    };
    let inner = inner.strip_suffix(']').ok_or(UrlError("unclosed bracket"))?;
    ip::ipv6(inner)
}

fn domain(raw: &str) -> Result<String, UrlError> {
    if raw.is_empty() {
        return Err(UrlError("empty host"));
    }
    let dotted: String = raw.chars().map(unify_dot).collect();
    let labels: Result<Vec<String>, UrlError> = dotted.split('.').map(label_to_ascii).collect();
    let host = labels?.join(".");
    if host.chars().any(forbidden) {
        return Err(UrlError("forbidden character in host"));
    }
    if ip::ends_in_number(&host) {
        return ip::ipv4(&host);
    }
    Ok(host)
}

fn unify_dot(c: char) -> char {
    match c {
        '\u{3002}' | '\u{ff61}' | '\u{ff0e}' => '.',
        _ => c,
    }
}

fn forbidden(c: char) -> bool {
    c.is_ascii_control() || " #%/:<>?@[\\]^|".contains(c)
}

fn label_to_ascii(label: &str) -> Result<String, UrlError> {
    if label.is_ascii() {
        return Ok(label.to_ascii_lowercase());
    }
    let mapped = map_label(label);
    if mapped.is_ascii() {
        return Ok(mapped);
    }
    let encoded = encode_label(&mapped).ok_or(UrlError("label too large"))?;
    Ok(format!("xn--{encoded}"))
}

/// NFKC, per-character lowercase (no context rules, as in UTS 46), NFKC again.
fn map_label(label: &str) -> String {
    let nfkc = ComposingNormalizerBorrowed::new_nfkc();
    let kept: String = nfkc.normalize(label).chars().filter(|c| !ignored(*c)).collect();
    let lower: String = kept.chars().flat_map(char::to_lowercase).collect();
    nfkc.normalize(&lower).into_owned()
}

fn ignored(c: char) -> bool {
    matches!(c, '\u{ad}' | '\u{200b}' | '\u{2060}' | '\u{feff}')
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
