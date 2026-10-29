//! A small URL front end for the three places that need one: a relay address (`normalize`)
//! and the host of a typed domain (`host`). It covers practical inputs of the `url` crate
//! (http/https, IDN hosts, IPv4/IPv6 literals) and refuses exotic ones.

mod host;
mod ip;
mod normalize;
mod punycode;

pub use host::host;
pub use normalize::normalize;

/// Why an address was refused. The text is for logs; callers show their own messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UrlError(&'static str);

impl std::fmt::Display for UrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for UrlError {}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod diff_tests;
