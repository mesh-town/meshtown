//! Tests for `dial.rs`: refusal for closed addresses and connection failure. The tests need no network:
//! the resolver is "dead", port 1 on the loopback isn't listening.

use std::collections::HashMap;
use std::net::Ipv4Addr;

use super::*;
use crate::exit::frame::{EXIT_CODE_ADDRESS_FORBIDDEN, EXIT_CODE_CONNECT_FAILED, EXIT_CODE_NO_ADDRESS};
use crate::exit::test_support::{exit_config, exit_config_with_dead_resolver, refusal_of};

#[path = "dial_literal_tests.rs"]
mod literal;
#[path = "dial_name_tests.rs"]
mod name;

mod open_tcp {
    use super::*;

    #[tokio::test]
    async fn closed_port_is_connect_failed() {
        let cfg = exit_config_with_dead_resolver();
        let closed = SocketAddr::from(([127, 0, 0, 1], 1));

        let refusal = refusal_of(open_tcp(&cfg, "closed.test", closed).await, "127.0.0.1:1");

        assert_eq!(
            refusal.code, EXIT_CODE_CONNECT_FAILED,
            "a closed port is a connection failure, not \"closed\""
        );
        assert!(
            refusal.line.contains("connection didn't open"),
            "the log line names the reason: {}",
            refusal.line
        );
    }
}

/// Refusal for a closed literal: code 7, the log line has the literal and no word "DNS".
fn then_forbidden_without_dns(refusal: &Refusal, literal: &str, why: &str) {
    assert_eq!(
        refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
        "{literal}: {why} (code 7)"
    );
    assert!(
        refusal.line.contains(literal),
        "{literal}: the log line names the literal: {}",
        refusal.line
    );
    assert!(
        !refusal.line.contains("DNS"),
        "{literal}: the literal does not go to DNS: {}",
        refusal.line
    );
}
