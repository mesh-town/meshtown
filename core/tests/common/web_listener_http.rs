//! The plain request helper of the web listener scenarios: `http` with the default 5 s limit.
//!
//! What it does: wraps `common::http_within`. What it does not do: it does not parse anything itself.
//! Links: included as a sibling of `common` by the scenarios that send raw requests; takes the base through `super::common`.

use std::time::Duration;

use super::common::{http_within, Resp};

/// Headers with an empty value ("Host", "") / ("Origin", "") are a signal to "not send this
/// header at all", suppressing the auto-add (used to really omit
/// `Origin`, rather than substitute its value).
pub(crate) fn http(
    port: u16,
    method: &str,
    path: &str,
    extra_headers: &[(&str, &str)],
    body: &[u8],
) -> Resp {
    http_within(
        port,
        method,
        path,
        extra_headers,
        body,
        Duration::from_secs(5),
    )
}
