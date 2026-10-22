use super::WebStartError;
use std::io::ErrorKind;

fn bind(port: u16, kind: ErrorKind, os: &str) -> WebStartError {
    WebStartError::Bind {
        port,
        reason: os.to_string(),
        kind,
    }
}

#[test]
fn busy_page_line() {
    let e = bind(
        7777,
        ErrorKind::AddrInUse,
        "Address already in use (os error 48)",
    );
    assert_eq!(
        e.continue_without_page_log_line().unwrap(),
        "the page didn't start: port 7777 is in use by another program — the node is running without the page; another port: meshtown run --web-port 7778"
    );
    assert!(e.to_string().contains("in use by another program"), "{e}");
}

#[test]
fn other_bind_line() {
    let e = bind(
        7777,
        ErrorKind::PermissionDenied,
        "Permission denied (os error 13)",
    );
    assert_eq!(
        e.continue_without_page_log_line().unwrap(),
        "the page didn't start: port 7777 didn't open (Permission denied (os error 13)) — the node is running without the page"
    );
    assert!(e.to_string().contains("didn't open"), "{e}");
}

#[test]
fn disabled_has_no_line() {
    assert!(WebStartError::Disabled
        .continue_without_page_log_line()
        .is_none());
}

#[test]
fn advice_only_for_port_causes() {
    let busy = bind(7777, ErrorKind::AddrInUse, "x")
        .start_failure_advice()
        .unwrap();
    assert!(busy.contains("--web-port 7778"), "{busy}");
    let other = bind(7777, ErrorKind::PermissionDenied, "x")
        .start_failure_advice()
        .unwrap();
    assert!(other.contains("--web-port"), "{other}");
    let off = WebStartError::Disabled.start_failure_advice().unwrap();
    assert!(
        off.contains("web.enabled") && !off.contains("--web-port"),
        "{off}"
    );
    let rnd = WebStartError::Random("x".into()).start_failure_advice();
    assert!(rnd.is_none_or(|t| !t.contains("--web-port")));
}
