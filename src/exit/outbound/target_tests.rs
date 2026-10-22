//! Tests for `target.rs`: choosing the destination addresses from resolver answers and the node's own address check.
//! The tests need no network.

use std::net::{Ipv4Addr, UdpSocket};
use std::time::Instant;

use super::*;
use crate::exit::frame::EXIT_CODE_NO_ADDRESS;
use crate::exit::test_support::refusal_of;

fn ip(text: &str) -> IpAddr {
    text.parse().expect("an IP address")
}

fn open_of(ips: &[&str]) -> Vec<SocketAddr> {
    allowed_addresses(ips.iter().map(|t| ip(t)), 443)
}

mod allowed {
    use super::*;

    #[test]
    fn keeps_answer_order_and_drops_closed() {
        let open = open_of(&["192.168.1.1", "93.184.216.34", "10.0.0.1", "1.1.1.1"]);

        assert_eq!(
            open,
            [
                SocketAddr::from(([93, 184, 216, 34], 443)),
                SocketAddr::from(([1, 1, 1, 1], 443))
            ]
        );
    }

    #[test]
    fn mapped_aaaa_is_judged_and_returned_as_ipv4() {
        let open = open_of(&["::ffff:127.0.0.1", "::ffff:93.184.216.34"]);

        assert_eq!(open, [SocketAddr::from(([93, 184, 216, 34], 443))]);
        assert!(open[0].is_ipv4(), "the mapped address is opened as plain IPv4");
    }

    #[test]
    fn closed_v6_is_dropped() {
        let open = open_of(&["::1", "fe80::1", "fc00::1", "2001:db8::1", "::127.0.0.1", "2606:4700::1111"]);

        assert_eq!(open, [SocketAddr::new(ip("2606:4700::1111"), 443)]);
    }

    #[test]
    fn at_most_four_candidates() {
        let open = open_of(&["1.1.1.1", "1.1.1.2", "1.1.1.3", "1.1.1.4", "1.1.1.5", "1.1.1.6"]);

        assert_eq!(open.len(), MAX_CANDIDATES);
        assert_eq!(open[3], SocketAddr::from(([1, 1, 1, 4], 443)), "the first four, in answer order");
    }

    #[test]
    fn own_address_is_dropped() {
        // the address the host uses to go outside is its own (no route — the probe is skipped)
        let Some(src) = outgoing_source() else {
            eprintln!("no default route — the outgoing address probe is skipped");
            return;
        };

        let open = allowed_addresses([src, ip("93.184.216.34")], 80);

        assert_eq!(open, [SocketAddr::from(([93, 184, 216, 34], 80))], "{src} is the node's own address");
    }
}

mod answers {
    use super::*;
    use crate::exit::test_support::{exit_config_with_dead_resolver, exit_config_with_silent_resolver};

    #[tokio::test]
    async fn v4_dns_error_is_no_address_with_dns_in_line() {
        let cfg = exit_config_with_dead_resolver();

        let refusal = refusal_of(v4_answer(&cfg.resolver, "down.test", 443).await, "down.test");

        assert_eq!(refusal.code, EXIT_CODE_NO_ADDRESS);
        assert!(refusal.line.contains("DNS"), "the log line names the reason: {}", refusal.line);
    }

    #[tokio::test]
    async fn v6_error_is_an_empty_answer() {
        let cfg = exit_config_with_dead_resolver();

        let answer = v6_answer(&cfg.resolver, "down.test", 443, Duration::from_secs(3)).await;

        assert!(answer.open.is_empty());
        assert!(!answer.any, "an error says nothing about addresses");
    }

    #[tokio::test]
    async fn v6_lookup_stops_at_wait() {
        let (cfg, _silent) = exit_config_with_silent_resolver();
        let started = Instant::now();

        let answer = v6_answer(&cfg.resolver, "slow.test", 443, Duration::from_millis(200)).await;

        assert!(answer.open.is_empty());
        assert!(started.elapsed() < Duration::from_millis(500), "the wait is bounded: {:?}", started.elapsed());
    }

    #[tokio::test]
    async fn v6_all_closed_reports_any() {
        let cfg = exit_config_with_dead_resolver();
        cfg.resolver.seed_cache_dual_for_tests("closed6.test", vec![], vec![ip6("fe80::1")]);

        let answer = v6_answer(&cfg.resolver, "closed6.test", 443, Duration::from_secs(3)).await;

        assert!(answer.open.is_empty());
        assert!(answer.any, "the answer had an address, it is only closed");
    }

    #[tokio::test]
    async fn v4_answer_marks_closed_addresses_as_any() {
        let cfg = exit_config_with_dead_resolver();
        cfg.resolver.seed_cache_for_tests("lan.test", vec![Ipv4Addr::new(10, 0, 0, 1)]);

        let answer = v4_answer(&cfg.resolver, "lan.test", 443).await.expect("answered from the cache");

        assert!(answer.open.is_empty() && answer.any && !answer.nx);
    }

    fn ip6(text: &str) -> std::net::Ipv6Addr {
        text.parse().expect("an IPv6 address")
    }
}

/// The address this host uses to go outside: a UDP `connect` only picks a route and sends
/// no packets. An error (no default route) gives `None`.
fn outgoing_source() -> Option<IpAddr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    sock.local_addr().ok().map(|addr| addr.ip())
}
