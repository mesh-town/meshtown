//! `Reply` and `ApiError`: the outcome of an exchange with the node page, before and after classification (`WEB-API.md`).
//! Classification `(status, body) → Reply` is in `classify.rs`; parsing of raw bytes is in `response.rs`.

use serde_json::Value;

/// The page's reply, classified per the contract (`WEB-API.md`).
#[derive(Debug)]
pub(crate) enum Reply {
    Ok(Value),
    Accepted,
    Unauthorized,
    Stopping,
    OtherStatus(u16),
}

/// A failure before classification: network, timeout or non-HTTP.
#[derive(Debug)]
pub(crate) enum ApiError {
    Refused,
    Timeout,
    NotHttp(String),
}
