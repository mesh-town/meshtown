//! Content coding of the static pages: parsing of `Accept-Encoding` and unpacking of the embedded
//! gzip bodies (a 10-byte header without optional fields, a raw deflate stream, an 8-byte trailer).

use axum::http::{header, HeaderMap};

#[cfg(test)]
#[path = "page_encoding_tests.rs"]
mod tests;

/// The embedded gzip body is malformed.
#[derive(Debug)]
pub(super) struct InflateError;

const HEADER_LEN: usize = 10;
const TRAILER_LEN: usize = 8;

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

/// Unpacks a gzip body made without optional header fields; the unpacked length must match `ISIZE`.
pub(super) fn inflate_gzip(gz: &[u8]) -> Result<Vec<u8>, InflateError> {
    let valid = gz.len() >= HEADER_LEN + TRAILER_LEN && gz[..4] == [0x1f, 0x8b, 0x08, 0];
    let body = valid
        .then(|| &gz[HEADER_LEN..gz.len() - TRAILER_LEN])
        .ok_or(InflateError)?;
    let out = miniz_oxide::inflate::decompress_to_vec(body).map_err(|_| InflateError)?;
    let isize_bytes: [u8; 4] = gz[gz.len() - 4..].try_into().map_err(|_| InflateError)?;
    (out.len() as u32 == u32::from_le_bytes(isize_bytes))
        .then_some(out)
        .ok_or(InflateError)
}
