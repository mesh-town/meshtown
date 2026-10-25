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
        assert!(
            open[0].is_ipv4(),
            "the mapped address is opened as plain IPv4"
        );
    }

    #[test]
    fn closed_v6_is_dropped() {
        let open = open_of(&[
            "::1",
            "fe80::1",
            "fc00::1",
            "2001:db8::1",
            "::127.0.0.1",
            "2606:4700::1111",
        ]);

        assert_eq!(open, [SocketAddr::new(ip("2606:4700::1111"), 443)]);
    }

    #[test]
    fn at_most_four_candidates() {
        let open = open_of(&[
            "1.1.1.1", "1.1.1.2", "1.1.1.3", "1.1.1.4", "1.1.1.5", "1.1.1.6",
        ]);

        assert_eq!(open.len(), MAX_CANDIDATES);
        assert_eq!(
            open[3],
            SocketAddr::from(([1, 1, 1, 4], 443)),
            "the first four, in answer order"
        );
    }

    #[test]
    fn own_address_is_dropped() {
        // the address the host uses to go outside is its own (no route — the probe is skipped)
        let Some(src) = outgoing_source() else {
            eprintln!("no default route — the outgoing address probe is skipped");
            return;
        };

        let open = allowed_addresses([src, ip("93.184.216.34")], 80);

        assert_eq!(
            open,
            [SocketAddr::from(([93, 184, 216, 34], 80))],
            "{src} is the node's own address"
        );
    }
}

mod answers {
    use super::*;
    use crate::exit::test_support::{
        exit_config_with_dead_resolver, exit_config_with_silent_resolver,
    };

    #[tokio::test]
    async fn v4_dns_error_is_no_address_with_dns_in_line() {
        let cfg = exit_config_with_dead_resolver();

        let refusal = refusal_of(
            v4_answer(&cfg.resolver, "down.test", 443).await,
            "down.test",
        );

        assert_eq!(refusal.code, EXIT_CODE_NO_ADDRESS);
        assert!(
            refusal.line.contains("DNS"),
            "the log line names the reason: {}",
            refusal.line
        );
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
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the wait is bounded: {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn v6_all_closed_reports_any() {
        let cfg = exit_config_with_dead_resolver();
        cfg.resolver
            .seed_cache_dual_for_tests("closed6.test", vec![], vec![ip6("fe80::1")]);

        let answer = v6_answer(&cfg.resolver, "closed6.test", 443, Duration::from_secs(3)).await;

        assert!(answer.open.is_empty());
        assert!(answer.any, "the answer had an address, it is only closed");
    }

    #[tokio::test]
    async fn v4_answer_marks_closed_addresses_as_any() {
        let cfg = exit_config_with_dead_resolver();
        cfg.resolver
            .seed_cache_for_tests("lan.test", vec![Ipv4Addr::new(10, 0, 0, 1)]);

        let answer = v4_answer(&cfg.resolver, "lan.test", 443)
            .await
            .expect("answered from the cache");

        assert!(answer.open.is_empty() && answer.any && !answer.nx);
    }

    fn ip6(text: &str) -> std::net::Ipv6Addr {
        text.parse().expect("an IPv6 address")
    }
}

mod udp {
    use super::*;
    use crate::exit::test_support::exit_config_with_dead_resolver;
    use crate::site_checks::doh::test_server::TestDoh;
    use simple_dns::TYPE;

    fn seeded(v4: Vec<Ipv4Addr>, v6: Vec<&str>) -> crate::exit::server::ExitConfig {
        let cfg = exit_config_with_dead_resolver();
        let v6 = v6
            .into_iter()
            .map(|t| t.parse().expect("an IPv6 address"))
            .collect();
        cfg.resolver.seed_cache_dual_for_tests("udp.test", v4, v6);
        cfg
    }

    #[tokio::test]
    async fn udp_address_prefers_ipv4() {
        let cfg = seeded(
            vec![Ipv4Addr::new(93, 184, 216, 34)],
            vec!["2606:4700::1111"],
        );

        let addr = udp_address(&cfg.resolver, "udp.test", 53).await;

        assert_eq!(addr, Some(SocketAddr::from(([93, 184, 216, 34], 53))));
    }

    #[tokio::test]
    async fn udp_address_falls_back_to_ipv6_for_aaaa_only_name() {
        let cfg = seeded(vec![], vec!["2606:4700::1111"]);

        let addr = udp_address(&cfg.resolver, "udp.test", 53).await;

        assert_eq!(addr, Some(SocketAddr::new(ip("2606:4700::1111"), 53)));
    }

    #[tokio::test]
    async fn udp_address_skips_aaaa_for_nonexistent_name() {
        let cfg = seeded(vec![], vec!["2606:4700::1111"]);
        let v4 = NameAnswer {
            open: vec![],
            any: false,
            nx: true,
        };

        assert_eq!(
            udp_address_from(&cfg.resolver, "udp.test", 53, v4).await,
            None
        );
    }

    #[tokio::test]
    async fn doh_nxdomain_stops_udp_lookup_before_aaaa() {
        let server = TestDoh::start().await;

        let addr = udp_address(&server.resolver(), "gone.nx.test", 53).await;

        assert_eq!(addr, None);
        assert_eq!(server.queries(), [("gone.nx.test".to_string(), TYPE::A)]);
    }

    #[tokio::test]
    async fn doh_empty_answer_still_asks_for_aaaa() {
        let server = TestDoh::start().await;

        let addr = udp_address(&server.resolver(), "a.empty.test", 53).await;

        assert_eq!(addr, None);
        let types: Vec<TYPE> = server.queries().into_iter().map(|q| q.1).collect();
        assert_eq!(types, [TYPE::A, TYPE::AAAA]);
    }

    #[tokio::test]
    async fn udp_address_is_none_when_all_closed() {
        let cfg = seeded(vec![Ipv4Addr::new(10, 0, 0, 1)], vec!["fe80::1"]);

        assert_eq!(udp_address(&cfg.resolver, "udp.test", 53).await, None);
    }
}

/// The address this host uses to go outside: a UDP `connect` only picks a route and sends
/// no packets. An error (no default route) gives `None`.
fn outgoing_source() -> Option<IpAddr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    sock.local_addr().ok().map(|addr| addr.ip())
}
