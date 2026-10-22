//! Tests for classifying collected steps into the final verdict (`classify.rs`).

use super::*;

fn passed() -> Steps {
    Steps {
        dns_ok: true,
        tcp: TcpStep::Ok,
        tls: TlsStep::Ok,
        http_status: 200,
        bytes_received: 5000,
        ..Steps::default()
    }
}

#[test]
fn everything_passed_is_ok() {
    let (v, r) = classify(&passed());
    assert_eq!(v, CheckVerdict::Ok);
    assert_eq!(r, "code 200");
}

#[test]
fn server_error_is_still_ok() {
    let s = Steps {
        http_status: 500,
        ..passed()
    };
    assert_eq!(classify(&s).0, CheckVerdict::Ok);
}

#[test]
fn tls_alert_is_network_blocked() {
    let s = Steps {
        tls: TlsStep::Alert("HandshakeFailure".to_string()),
        ..passed()
    };
    let (v, r) = classify(&s);
    assert_eq!(v, CheckVerdict::NetworkBlocked);
    assert!(r.contains("HandshakeFailure"), "{r}");
}

#[test]
fn tcp_reset_is_network_blocked_and_timeout_is_down() {
    let reset = Steps {
        tcp: TcpStep::Reset,
        tls: TlsStep::NotTried,
        ..passed()
    };
    assert_eq!(classify(&reset).0, CheckVerdict::NetworkBlocked);
    let timed = Steps {
        tcp: TcpStep::Timeout,
        tls: TlsStep::NotTried,
        ..passed()
    };
    assert_eq!(classify(&timed).0, CheckVerdict::Down);
}

#[test]
fn stall_inside_band_is_freeze() {
    let s = Steps {
        stalled: true,
        bytes_received: 20 * 1024,
        ..passed()
    };
    let (v, r) = classify(&s);
    assert_eq!(v, CheckVerdict::NetworkBlocked);
    assert_eq!(r, "freeze at 20 KB");
}

#[test]
fn stall_outside_band_is_down() {
    let small = Steps {
        stalled: true,
        bytes_received: 2 * 1024,
        ..passed()
    };
    assert_eq!(classify(&small).0, CheckVerdict::Down);
    let big = Steps {
        stalled: true,
        bytes_received: 50 * 1024,
        ..passed()
    };
    assert_eq!(classify(&big).0, CheckVerdict::Down);
}

#[test]
fn geoblock_and_antibot_are_separate() {
    let geo = Steps {
        http_status: 451,
        ..passed()
    };
    assert_eq!(classify(&geo).0, CheckVerdict::SiteGeoblock);
    let bot = Steps {
        http_status: 403,
        body_mark: BodyMark::Antibot,
        ..passed()
    };
    assert_eq!(classify(&bot).0, CheckVerdict::Antibot);
    let forbidden = Steps {
        http_status: 403,
        ..passed()
    };
    assert_eq!(classify(&forbidden).0, CheckVerdict::SiteGeoblock);
}

#[test]
fn mismatch_wins_only_when_the_rest_passed() {
    let ok = Steps {
        dns_mismatch: true,
        ..passed()
    };
    assert_eq!(classify(&ok).0, CheckVerdict::DnsMismatch);
    let blocked = Steps {
        dns_mismatch: true,
        tls: TlsStep::Closed,
        ..passed()
    };
    assert_eq!(classify(&blocked).0, CheckVerdict::NetworkBlocked);
    let fake = Steps {
        unusable_addr: true,
        ..passed()
    };
    assert_eq!(classify(&fake).0, CheckVerdict::DnsMismatch);
}
