//! Pure parsing of the raw response bytes: the start of the status line, the end of the headers within the first 16 KiB,
//! the body length by `Content-Length` (or up to the end of the bytes received), an upper bound of 1 MiB on the body,
//! `Transfer-Encoding` is not supported. The bytes of a foreign response are not copied into the error text.

use super::types::ApiError;

/// We do not look for headers beyond this bound — `NotHttp`.
pub(crate) const MAX_HEADER_BYTES: usize = 16 * 1024;
/// We do not accept a body beyond this bound — `NotHttp`.
pub(crate) const MAX_BODY_BYTES: usize = 1024 * 1024;

pub(crate) fn parse_response(bytes: &[u8]) -> Result<(u16, Vec<u8>), ApiError> {
    let header_end = find_header_end(bytes)?;
    let header = &bytes[..header_end];
    let rest = &bytes[header_end..];

    let status = parse_status_line(header)?;

    if has_header(header, "transfer-encoding") {
        return Err(ApiError::NotHttp(
            "Transfer-Encoding is not supported".to_string(),
        ));
    }

    let body = match content_length(header) {
        Some(len) => {
            if len > MAX_BODY_BYTES {
                return Err(ApiError::NotHttp(
                    "response body is larger than 1 MiB".to_string(),
                ));
            }
            if rest.len() < len {
                return Err(ApiError::NotHttp("response was cut off".to_string()));
            }
            rest[..len].to_vec()
        }
        None => {
            if rest.len() > MAX_BODY_BYTES {
                return Err(ApiError::NotHttp(
                    "response body is larger than 1 MiB".to_string(),
                ));
            }
            rest.to_vec()
        }
    };

    Ok((status, body))
}

/// The position of the first byte after `\r\n\r\n`, if it is within the first `MAX_HEADER_BYTES` bytes.
fn find_header_end(bytes: &[u8]) -> Result<usize, ApiError> {
    let window_len = bytes.len().min(MAX_HEADER_BYTES);
    bytes[..window_len]
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
        .ok_or_else(|| ApiError::NotHttp("no end of headers".to_string()))
}

/// The status line: `HTTP/1.0 ` or `HTTP/1.1 ` and a three-digit code.
fn parse_status_line(header: &[u8]) -> Result<u16, ApiError> {
    let not_http = || ApiError::NotHttp("not an HTTP response".to_string());
    let line_end = header
        .iter()
        .position(|&b| b == b'\r')
        .unwrap_or(header.len());
    let line = std::str::from_utf8(&header[..line_end]).map_err(|_| not_http())?;
    let rest = line
        .strip_prefix("HTTP/1.0 ")
        .or_else(|| line.strip_prefix("HTTP/1.1 "))
        .ok_or_else(not_http)?;
    let code = rest.get(..3).ok_or_else(not_http)?;
    if code.bytes().all(|b| b.is_ascii_digit()) {
        code.parse::<u16>().map_err(|_| not_http())
    } else {
        Err(not_http())
    }
}

/// The header lines without the start line and without the trailing empty lines from `\r\n\r\n`.
fn header_lines(header: &[u8]) -> impl Iterator<Item = &str> {
    std::str::from_utf8(header)
        .unwrap_or("")
        .split("\r\n")
        .skip(1)
        .filter(|line| !line.is_empty())
}

fn content_length(header: &[u8]) -> Option<usize> {
    header_lines(header).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    })
}

fn has_header(header: &[u8], name: &str) -> bool {
    header_lines(header).any(|line| {
        line.split_once(':')
            .map(|(n, _)| n.eq_ignore_ascii_case(name))
            .unwrap_or(false)
    })
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
