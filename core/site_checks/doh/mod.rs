//! Own resolver for the checks: DNS over HTTPS and a separate query to the ISP's resolver.
//!
//! What it does: builds and parses DNS packets, asks a DoH server given by IP address for a domain's address,
//! and — only as a spoofing signal — asks the ISP's resolver over UDP-53;
//! drops addresses that must not be used (own fake-ip pool, loopback, "zero", link-local).
//! Caches addresses only for the exit (`resolve_cached` for IPv4, `resolve_cached_v6` for IPv6, one entry per name,
//! answer TTL within 30–120 s, up to 1024 names); reachability checks query around the cache (`resolve`).
//! What it doesn't do: doesn't use the system resolver, doesn't remember failures, remembers an empty
//! answer only next to a non-empty one of the other family, queries AAAA only on request of `resolve_cached_v6`
//! (the checks and the ISP query ask A only), doesn't verify DNSSEC signatures.
//! Related: transport is `crate::site_checks::http_min` over TLS; used by `crate::site_checks::reachability` and
//! `crate::exit`.
//!
//! Files: `resolver` — `Resolver` (DoH and UDP queries, the cache owner); `packet` — building queries and parsing
//! answers; `unusable` — addresses a check must not use; `cache` — the address cache.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

mod cache;
mod packet;
mod resolver;
mod unusable;

#[cfg(test)]
pub(crate) mod test_server;
#[cfg(test)]
#[path = "test_server_tests.rs"]
mod test_server_tests;

pub use resolver::{DnsAnswer, Resolver};
pub use unusable::{is_fake_ip, is_unusable};

/// Default DoH server: Cloudflare, given by address — no name needs to be resolved.
pub const DEFAULT_DOH: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 443);
/// DoH request path (RFC 8484).
pub const DOH_PATH: &str = "/dns-query";
/// Content type of the DoH request and response.
pub const DNS_MESSAGE: &str = "application/dns-message";
