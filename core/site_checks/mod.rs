//! Site checks: whether a domain opens, and how.
//!
//! - `reachability` - the step-by-step domain check and result classes.
//! - `checks` - `chk/` records and the check schedule.
//! - `doh` - own resolver for checks (DNS over HTTPS).
//! - `http_min` - minimal HTTP/1.1 client and TLS setup for checks.
//! - `geoip` - the country of an address.

#[cfg(feature = "net-hyper")]
pub mod checks;
pub mod doh;
#[cfg(feature = "net-hyper")]
pub mod geoip;
pub mod http_min;
pub mod reachability;
