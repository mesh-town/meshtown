//! Tests for `target.rs`: choosing the destination address from a resolver answer and the node's own address check.
//! The tests need no network.

use std::net::UdpSocket;

use super::*;
use crate::exit::frame::{EXIT_CODE_ADDRESS_FORBIDDEN, EXIT_CODE_NO_ADDRESS};
use crate::exit::test_support::refusal_of;

mod named_address {
    use super::*;

    #[test]
    fn all_closed_answer_is_forbidden_without_address_in_line() {
        let answer = [Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(127, 0, 0, 1)];

        let refusal = refusal_of(
            first_allowed_resolved_address(&answer, "router.example", 443),
            "router.example",
        );

        assert_eq!(
            refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
            "all addresses in the answer are closed — refusal with code 7"
        );
        assert!(
            refusal.line.contains("router.example"),
            "the log line names the host: {}",
            refusal.line
        );
        assert!(
            !refusal.line.contains("192.168") && !refusal.line.contains("127.0.0.1"),
            "the address from the DoH answer is not given out in the log: {}",
            refusal.line
        );
    }

    #[test]
    fn mixed_answer_picks_first_open() {
        let answer = [
            Ipv4Addr::new(192, 168, 1, 1),
            Ipv4Addr::new(93, 184, 216, 34),
        ];

        let addr = first_allowed_resolved_address(&answer, "mixed.example", 443)
            .expect("there is an open address among them — it gets chosen");

        assert_eq!(addr, SocketAddr::from(([93, 184, 216, 34], 443)));
    }

    #[test]
    fn empty_answer_is_no_address() {
        let refusal = refusal_of(first_allowed_resolved_address(&[], "empty.example", 443), "empty.example");

        assert_eq!(
            refusal.code, EXIT_CODE_NO_ADDRESS,
            "an empty DoH answer gives \"no usable address\", not \"closed\""
        );
    }
}

mod not_own {
    use super::*;

    #[test]
    fn own_address_is_forbidden_before_connect() {
        // the loopback is the node's own address in any environment
        let loopback = SocketAddr::from(([127, 0, 0, 1], 80));
        let refusal = refusal_of(reject_own_address(loopback, "self.test"), "127.0.0.1:80");
        assert_eq!(
            refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
            "the loopback is the node's own address, refusal with code 7"
        );

        // the address the host uses to go outside is also its own (no route — the probe is skipped)
        let Some(src) = outgoing_source() else {
            eprintln!("no default route — the outgoing address probe is skipped");
            return;
        };
        let refusal = refusal_of(
            reject_own_address(SocketAddr::new(src, 80), "own.test"),
            "outgoing address",
        );
        assert_eq!(
            refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
            "{src}: the node's outgoing address is its own, refusal with code 7"
        );
        assert!(
            refusal.line.contains("own.test"),
            "the log line names the host: {}",
            refusal.line
        );
        assert!(
            !refusal.line.contains(&src.to_string()),
            "the own address is not given out in the log: {}",
            refusal.line
        );
    }

    #[test]
    fn foreign_address_passes_own_check() {
        let foreign = SocketAddr::from(([192, 0, 2, 1], 80));

        let passed = reject_own_address(foreign, "x").expect("a foreign address passes the own-address check");

        assert_eq!(passed, foreign, "the address is returned unchanged");
    }
}

/// The address this host uses to go outside: a UDP `connect` only picks a route and sends
/// no packets. An error (no default route) gives `None`.
fn outgoing_source() -> Option<IpAddr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    sock.local_addr().ok().map(|addr| addr.ip())
}
