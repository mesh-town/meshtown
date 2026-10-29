//! Unpacking of the embedded gzip bodies (pages, word list): a 10-byte header without optional
//! fields, a raw deflate stream, an 8-byte trailer.

use std::fmt;

#[cfg(test)]
#[path = "gzip_tests.rs"]
mod tests;

/// The embedded gzip body is malformed.
#[derive(Debug)]
pub struct InflateError;

impl fmt::Display for InflateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("malformed gzip body")
    }
}

impl std::error::Error for InflateError {}

const HEADER_LEN: usize = 10;
const TRAILER_LEN: usize = 8;

/// Unpacks a gzip body made without optional header fields; the unpacked length must match `ISIZE`.
pub fn inflate_gzip(gz: &[u8]) -> Result<Vec<u8>, InflateError> {
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
