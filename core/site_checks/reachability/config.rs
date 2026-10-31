//! Check settings (`CheckConfig`) and constants: record version, the national DPI filter freeze band,
//! download ceiling, client string, markers of stub pages.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::site_checks::http_min::HttpLimits;

/// Version of the `chk/` record format.
pub const CHECK_RECORD_VERSION: u8 = 1;
/// Lower bound of the national DPI filter freeze band.
pub const FREEZE_MIN_BYTES: u32 = 12 * 1024;
/// Upper bound of the national DPI filter freeze band.
pub const FREEZE_MAX_BYTES: u32 = 36 * 1024;
/// How much to download so the freeze has time to show.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Markers of the robot-check page.
pub(super) const ANTIBOT_MARKERS: [&str; 4] = [
    "just a moment",
    "challenge-platform",
    "checking your browser",
    "cf-chl",
];
/// Markers of the "not available in your region" stub.
pub(super) const GEO_MARKERS: [&str; 3] = [
    "not available in your",
    "unavailable in your country",
    "geo-restricted",
];

/// The client string in check requests: an ordinary browser, with no node or user data.
/// It lives here rather than in `checks` because `CheckConfig::default()` takes it, and this module
/// is added a step before `checks` appears.
pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// Check settings.
#[derive(Debug, Clone)]
pub struct CheckConfig {
    pub limits: HttpLimits,
    pub user_agent: String,
    /// The provider's resolver for the spoofing sign; `None` — the sign is not collected.
    pub provider_dns: Option<SocketAddr>,
    /// Domains marked with the `--block-site` flag: the check doesn't run for them but at once
    /// returns the "blocked by network" class. This lets a route through a member be
    /// checked where nothing is actually blocked.
    pub blocked_domains: Arc<HashSet<String>>,
    /// Extra phrases of the region stub from the settings file, already lowercase.
    pub geo_markers: Arc<[String]>,
}

impl Default for CheckConfig {
    fn default() -> Self {
        CheckConfig {
            limits: HttpLimits {
                max_bytes: MAX_BODY_BYTES,
                ..HttpLimits::default()
            },
            user_agent: DEFAULT_USER_AGENT.to_string(),
            provider_dns: None,
            blocked_domains: Arc::new(HashSet::new()),
            geo_markers: Arc::from(Vec::new()),
        }
    }
}
