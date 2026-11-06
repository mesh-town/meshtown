//! Step-by-step check of a domain's reachability and assigning the result to a class.
//!
//! What it does: for one domain it runs DNS (own DoH), TCP, TLS with the real server name
//! and HTTP GET counting the bytes received (`probe.rs`), or the same through a member's exit (`probe_exit.rs`); a separate pure function assigns
//! the collected steps to one of the classes: ok, DNS spoofing, blocked by network, site geoblock, bot
//! protection, down (`classify.rs`).
//! Files: `config.rs` — settings and constants, `verdict.rs` — the result class, `steps.rs` —
//! intermediate step results, `record.rs` — the `chk/<domain>/` record, `classify.rs` — pure
//! classification, `probe.rs` — direct network steps and building the record, `probe_stream.rs` — TLS and HTTP over any byte stream,
//! `probe_exit.rs` — the same check through a member's exit.
//! What it doesn't do: it doesn't use the system resolver, doesn't merge results of different nodes, doesn't build
//! a route, doesn't parse a spoofed certificate, doesn't check AAAA records.
//! Links: the resolver is `crate::site_checks::doh`, the transport is `crate::site_checks::http_min`; the record is published by
//! `crate::site_checks::checks`.

mod classify;
mod config;
mod probe;
#[cfg(feature = "net-hyper")]
mod probe_exit;
mod probe_stream;
mod record;
mod steps;
mod verdict;

pub use classify::classify;
pub use config::{
    CheckConfig, CHECK_RECORD_VERSION, DEFAULT_USER_AGENT, FREEZE_MAX_BYTES, FREEZE_MIN_BYTES,
    MAX_BODY_BYTES,
};
pub use probe::check_domain;
#[cfg(feature = "net-hyper")]
pub use probe_exit::check_domain_via_exit;
pub use record::CheckRecord;
pub use steps::{BodyMark, Steps, TcpStep, TlsStep};
pub use verdict::CheckVerdict;
