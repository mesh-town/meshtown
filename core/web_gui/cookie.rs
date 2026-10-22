//! Page session cookie: name from the node key, session value, request parsing.

use crate::transport::net::PeerId;
use axum::http::{header, HeaderMap, HeaderValue};

use super::auth::tickets::SESSION_TTL;

/// Cookie lifetime — the same as the session's in the table; there is no separate number.
pub(crate) const SESSION_MAX_AGE_SEC: u64 = SESSION_TTL.as_secs();

const NAME_PREFIX: &str = "meshtown_";
const NAME_KEY_CHARS: usize = 10;

/// `meshtown_` and the first 10 characters of the node's public key (`[0-9a-f]`): two nodes on one
/// host have different ports, but cookies don't distinguish ports — the name tells them apart.
pub(crate) fn session_cookie_name(node_id: &PeerId) -> String {
    let full = node_id.to_string();
    format!("{NAME_PREFIX}{}", &full[..NAME_KEY_CHARS])
}

fn with_attrs(name: &str, value: &str, max_age: u64) -> HeaderValue {
    let line = format!("{name}={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}");
    HeaderValue::from_str(&line).unwrap_or_else(|_| HeaderValue::from_static(""))
}

pub(crate) fn session_cookie_header(name: &str, id: &str) -> HeaderValue {
    with_attrs(name, id, SESSION_MAX_AGE_SEC)
}

pub(crate) fn expired_session_cookie_header(name: &str) -> HeaderValue {
    with_attrs(name, "", 0)
}

fn pair_value<'a>(pair: &'a str, name: &str) -> Option<&'a str> {
    let (n, v) = pair.trim().split_once('=')?;
    (n == name).then_some(v)
}

/// The first match by exact name among all `Cookie` headers; a value of the wrong
/// form is skipped.
pub(crate) fn session_id(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair_value(pair, name))
        .find(|v| super::token::is_well_formed(v))
        .map(str::to_string)
}

#[cfg(test)]
#[path = "cookie_tests.rs"]
mod tests;
