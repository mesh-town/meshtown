//! Pure classification of collected steps (`Steps`) into a result class (`CheckVerdict`) — no network.
//! The order of the checks below sets the class priority when several signs match.

use super::config::{FREEZE_MAX_BYTES, FREEZE_MIN_BYTES};
use super::steps::{BodyMark, Steps, TcpStep, TlsStep};
use super::verdict::CheckVerdict;

/// The DNS part: an unusable address or an empty answer. `None` — DNS is fine, classification continues.
fn classify_dns(steps: &Steps) -> Option<(CheckVerdict, String)> {
    if steps.unusable_addr {
        return Some((
            CheckVerdict::DnsMismatch,
            "answer from the fake-ip pool or an unusable address".to_string(),
        ));
    }
    if !steps.dns_ok {
        return Some((CheckVerdict::Down, "DNS returned no address".to_string()));
    }
    None
}

/// TCP and TLS: the network may have refused before the site did. `None` — both steps passed, classification continues.
fn classify_transport(steps: &Steps) -> Option<(CheckVerdict, String)> {
    match steps.tcp {
        TcpStep::Refused => {
            return Some((CheckVerdict::NetworkBlocked, "TCP: refused".to_string()))
        }
        TcpStep::Reset => return Some((CheckVerdict::NetworkBlocked, "TCP: reset".to_string())),
        TcpStep::Timeout => return Some((CheckVerdict::Down, "TCP: timeout".to_string())),
        TcpStep::NotTried => return Some((CheckVerdict::Down, "TCP not tried".to_string())),
        TcpStep::Ok => {}
    }
    match &steps.tls {
        TlsStep::Alert(a) => {
            return Some((
                CheckVerdict::NetworkBlocked,
                format!("TLS with the real name: alert {a}"),
            ))
        }
        TlsStep::Closed => {
            return Some((
                CheckVerdict::NetworkBlocked,
                "TLS with the real name: connection dropped".to_string(),
            ))
        }
        TlsStep::Timeout => {
            return Some((
                CheckVerdict::NetworkBlocked,
                "TLS with the real name: timeout".to_string(),
            ))
        }
        TlsStep::Invalid(e) => {
            return Some((
                CheckVerdict::NetworkBlocked,
                format!("TLS with the real name: {e}"),
            ))
        }
        TlsStep::NotTried => return Some((CheckVerdict::Down, "TLS not tried".to_string())),
        TlsStep::Ok => {}
    }
    None
}

/// The response body: national DPI filter freeze, read cutoff, robot check or geoblock. `None` — nothing matched.
fn classify_response(steps: &Steps) -> Option<(CheckVerdict, String)> {
    if steps.stalled
        && steps.bytes_received >= FREEZE_MIN_BYTES
        && steps.bytes_received <= FREEZE_MAX_BYTES
    {
        return Some((
            CheckVerdict::NetworkBlocked,
            format!("freeze at {} KB", steps.bytes_received / 1024),
        ));
    }
    if steps.http_status == 0 {
        return Some((CheckVerdict::Down, "no HTTP response arrived".to_string()));
    }
    if steps.stalled {
        return Some((
            CheckVerdict::Down,
            format!("read hung at {} KB", steps.bytes_received / 1024),
        ));
    }
    if steps.body_mark == BodyMark::Antibot {
        return Some((
            CheckVerdict::Antibot,
            format!("robot check, code {}", steps.http_status),
        ));
    }
    if steps.http_status == 451 {
        return Some((CheckVerdict::SiteGeoblock, "code 451".to_string()));
    }
    if steps.http_status == 403 || steps.body_mark == BodyMark::Geo {
        return Some((
            CheckVerdict::SiteGeoblock,
            format!("site refused after TLS, code {}", steps.http_status),
        ));
    }
    None
}

/// Assigns the collected steps to a class. A pure function: no network inside.
pub fn classify(steps: &Steps) -> (CheckVerdict, String) {
    if let Some(r) = classify_dns(steps) {
        return r;
    }
    if let Some(r) = classify_transport(steps) {
        return r;
    }
    if let Some(r) = classify_response(steps) {
        return r;
    }
    if steps.dns_mismatch {
        return (
            CheckVerdict::DnsMismatch,
            "provider and DoH addresses don't overlap".to_string(),
        );
    }
    (CheckVerdict::Ok, format!("code {}", steps.http_status))
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
