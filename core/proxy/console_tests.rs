//! Tests of console line parsing for the proxy (`console.rs`).

use super::*;

#[test]
fn parse_proxy_line_reads_four_commands() {
    assert_eq!(parse_proxy_line("socks").unwrap().unwrap(), ProxyCommand::Socks);
    assert_eq!(
        parse_proxy_line("route example.com").unwrap().unwrap(),
        ProxyCommand::Route("example.com".to_string())
    );
    assert_eq!(
        parse_proxy_line("get example.com").unwrap().unwrap(),
        ProxyCommand::Get("example.com".to_string())
    );
    assert_eq!(parse_proxy_line("exit on").unwrap().unwrap(), ProxyCommand::Exit(true));
    assert_eq!(parse_proxy_line("exit off").unwrap().unwrap(), ProxyCommand::Exit(false));
}

#[test]
fn parse_proxy_line_reports_wrong_arguments() {
    assert!(parse_proxy_line("route").unwrap().is_err());
    assert!(parse_proxy_line("get a b").unwrap().is_err());
    assert!(parse_proxy_line("exit maybe").unwrap().is_err());
    assert!(parse_proxy_line("socks now").unwrap().is_err());
}

#[test]
fn parse_proxy_line_ignores_alien_lines() {
    assert!(parse_proxy_line("domains list").is_none());
    assert!(parse_proxy_line("checks").is_none());
    assert!(parse_proxy_line("").is_none());
}
