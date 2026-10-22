//! The check result line for output: a summary one (`describe`) and one with per-step times
//! (`describe_steps`, only for `[check]` lines about a single domain).

use crate::site_checks::reachability::CheckRecord;

/// The result line for output.
pub fn describe(record: &CheckRecord) -> String {
    let total =
        record.dns_ms as u32 + record.tcp_ms as u32 + record.tls_ms as u32 + record.http_ms as u32;
    format!(
        "{} ({}), {} ms",
        record.verdict.text(),
        record.reason,
        total
    )
}

/// The same line, but with the times of individual steps: which step the domain got stuck on
/// is not visible from the total. Needed only by `[check]` lines about a single domain.
pub fn describe_steps(record: &CheckRecord) -> String {
    format!(
        "{} — DNS {}, TCP {}, TLS {}, HTTP {} ms, {} bytes",
        describe(record),
        record.dns_ms,
        record.tcp_ms,
        record.tls_ms,
        record.http_ms,
        record.bytes
    )
}
