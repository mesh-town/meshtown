//! Tests of the exit codes and of the error report (`exit.rs`).

use super::*;
use crate::proxy::BindError;

fn busy(port: u16) -> anyhow::Error {
    anyhow::Error::new(BindError {
        port,
        io_error_kind: std::io::ErrorKind::AddrInUse,
        os_message: "Address already in use (os error 48)".into(),
    })
}

fn is_code(e: &anyhow::Error, want: u8) -> bool {
    format!("{:?}", exit_code(e)) == format!("{:?}", ExitCode::from(want))
}

#[test]
fn not_running_in_chain_is_3() {
    let e = anyhow::Error::new(NotRunning { port: 7777 }).context("asking for status");
    assert!(is_code(&e, 3));
}

#[test]
fn usage_error_under_context_is_2() {
    let e = anyhow::Error::new(UsageError("bad".into())).context("in a command");
    assert!(is_code(&e, 2));
}

#[test]
fn other_error_is_1() {
    assert!(is_code(&anyhow::anyhow!("boom"), 1));
}

#[test]
fn not_running_text_names_port_and_web_port_flag() {
    let text = NotRunning { port: 4242 }.to_string();
    assert!(text.contains("isn't running on port 4242"), "{text}");
    assert!(text.contains("--web-port <PORT>"), "{text}");
}

#[test]
fn report_joins_causes() {
    let e = anyhow::anyhow!("a").context("b");
    assert_eq!(error_report(&e), "Error: b: a");
}

#[test]
fn report_keeps_own_prefix() {
    let e = anyhow::anyhow!("x").context("Error: already marked");
    let text = error_report(&e);
    assert_eq!(text, "Error: already marked: x");
}

#[test]
fn report_adds_prefix_to_text_starting_with_error_word() {
    let e = anyhow::anyhow!("x").context("Errors in the file");
    assert_eq!(error_report(&e), "Error: Errors in the file: x");
}

#[test]
fn report_adds_proxy_advice() {
    let text = error_report(&busy(1080));
    assert_eq!(
        text,
        "Error: port 1080 for the browser proxy is in use by another program (Address already in use (os error 48)).\n\
         Close it or start Mesh Town on another port: meshtown run --socks-port 1081"
    );
}

#[test]
fn report_adds_advice_under_context() {
    let text = error_report(&busy(1080).context("starting the node"));
    assert!(text.starts_with("Error: starting the node: port 1080"));
    assert!(text.ends_with("--socks-port 1081"));
    assert_eq!(text.lines().count(), 2);
}
