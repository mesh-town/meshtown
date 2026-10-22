//! Time in the format the settings file uses: `saved_at` and the `.bad-<stamp>` marker.

/// Converts Unix seconds to (year, month, day, hour, minute, second) in UTC. The `civil_from_days` algorithm
/// by Howard Hinnant: days since the epoch → civil date.
pub fn utc_parts(unix_secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (unix_secs / 86_400) as i64;
    let rem = (unix_secs % 86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

/// `2026-09-24T19:41:07Z` — for `saved_at`.
pub fn iso_utc(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = utc_parts(unix_secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// `20260924-194107` — for `config.json.bad-…`.
pub fn stamp_utc(unix_secs: u64) -> String {
    let (y, mo, d, h, mi, s) = utc_parts(unix_secs);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

#[cfg(test)]
#[path = "time_tests.rs"]
mod tests;
