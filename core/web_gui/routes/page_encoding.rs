//! Content coding of the static pages: parsing of `Accept-Encoding` and the content coding of the
//! embedded gzip bodies (unpacking is `crate::gzip`).

use axum::http::{header, HeaderMap};

#[cfg(test)]
#[path = "page_encoding_tests.rs"]
mod tests;

/// Whether the client accepts `gzip`: an element `gzip` or `*` with a non-zero `q` in any
/// `Accept-Encoding` line; an explicit `gzip;q=0` is a refusal even next to `*`.
pub(super) fn accepts_gzip(headers: &HeaderMap) -> bool {
    let elements: Vec<(String, bool)> = headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|line| line.split(','))
        .map(element)
        .collect();
    let find = |name: &str| elements.iter().find(|e| e.0 == name).map(|e| e.1);
    find("gzip").or_else(|| find("*")).unwrap_or(false)
}

/// (lower-case coding name, whether `q` is non-zero) of one list element.
fn element(raw: &str) -> (String, bool) {
    let mut parts = raw.split(';');
    let name = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let weighted = parts
        .filter_map(|p| p.split_once('='))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("q"));
    (name, weighted.is_none_or(|(_, q)| q_is_non_zero(q.trim())))
}

fn q_is_non_zero(q: &str) -> bool {
    q.chars().any(|c| c.is_ascii_digit() && c != '0')
}
