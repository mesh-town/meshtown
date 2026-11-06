//! Tests of the check through an exit: the record built from the exit's reply code (`probe_exit.rs`).

use super::*;
use crate::exit::{EXIT_CODE_ADDRESS_FORBIDDEN, EXIT_CODE_REFUSED};
use crate::site_checks::reachability::CheckVerdict;

#[test]
fn no_address_code_gives_down() {
    let record = record_for_code(EXIT_CODE_NO_ADDRESS, 120).unwrap();
    assert_eq!(record.verdict, CheckVerdict::Down);
    assert_eq!(record.dns_ms, 120);
    assert_eq!(record.tcp_ms, 0);
}

#[test]
fn connect_failed_code_gives_tcp_timeout_step() {
    let record = record_for_code(EXIT_CODE_CONNECT_FAILED, 8000).unwrap();
    let steps = Steps {
        dns_ok: true,
        tcp: TcpStep::Timeout,
        ..Steps::default()
    };
    assert_eq!(
        record.verdict,
        crate::site_checks::reachability::classify(&steps).0
    );
    assert_eq!(record.tcp_ms, 8000);
}

#[test]
fn other_codes_are_errors_not_records() {
    for code in [EXIT_CODE_REFUSED, EXIT_CODE_ADDRESS_FORBIDDEN, 200] {
        assert!(record_for_code(code, 5).is_err(), "code {code}");
    }
}
