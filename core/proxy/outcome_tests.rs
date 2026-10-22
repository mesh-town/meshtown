//! Tests for the shape of the connection log summary line (`outcome.rs`).

use super::*;

#[test]
fn outcome_line_keeps_the_agreed_shape() {
    let done = ProxyOutcome::success("via 2f9a1c8d", 1256, 517);
    assert_eq!(
        outcome_line("example.com:443", &done, 842),
        "example.com:443 via 2f9a1c8d: success, received 1256 bytes, sent 517 bytes, 842 ms"
    );
    let refused = ProxyOutcome::refused(
        "no route",
        "domain is marked --block-domain, no member with access",
    );
    assert_eq!(
        outcome_line("example.com:443", &refused, 2),
        "example.com:443 no route: refused: domain is marked --block-domain, \
         no member with access, received 0 bytes, sent 0 bytes, 2 ms"
    );
}

/// `ProxyOutcome::interrupted` (removing an exit in the middle of a connection): the line names
/// the reason "interrupted: exit no longer selected" and the bytes are 0, the `splice` counters are lost together with
/// the interrupted task.
#[test]
fn interrupted_line_says_prervano_with_zero_bytes() {
    let interrupted = ProxyOutcome::interrupted("exit ab12");
    let line = outcome_line("example.com:443", &interrupted, 5);
    assert!(
        line.contains("interrupted: exit no longer selected"),
        "the line must name the reason for the interruption: {line}"
    );
    assert!(
        line.contains("received 0 bytes"),
        "the bytes must be zero — the splice counters are lost: {line}"
    );
}

mod client_left_before_path {
    use super::*;

    #[test]
    fn abandoned_line_says_client_left() {
        // given: the SOCKS client left before a route was found
        let outcome = ProxyOutcome::abandoned();

        // when: the summary line is built
        let line = outcome_line("t.test", &outcome, 1500);

        // then: the route is "no route", the reason is named, the bytes are zero
        assert_eq!(
            line,
            "t.test no route: interrupted: client left before a route was found, received 0 bytes, \
             sent 0 bytes, 1500 ms"
        );
    }
}
