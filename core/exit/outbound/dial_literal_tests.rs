//! Tests for `dial.rs`: IP literals and the test table `host_overrides` (`open_target_with`).

use super::*;

#[tokio::test]
async fn ipv4_literal_skips_dns() {
    let cfg = exit_config_with_dead_resolver();

    let addr = dial(&cfg, "1.2.3.4", 443)
        .await
        .expect("an IPv4 literal resolves without touching the resolver");

    assert_eq!(addr, SocketAddr::from(([1, 2, 3, 4], 443)));
}

#[tokio::test]
async fn loopback_v4_literal_is_forbidden_without_dns() {
    // given: the resolver is "dead" — any DNS query would end with a "DNS" line
    let cfg = exit_config_with_dead_resolver();

    for literal in ["127.0.0.1", "0.0.0.0"] {
        // when: a member asks the exit to open a connection to the exit's loopback
        let refusal = refusal_of(dial(&cfg, literal, 80).await, literal);

        // then: refusal with code 7 without going to DNS
        then_forbidden_without_dns(
            &refusal,
            literal,
            "the loopback — the exit's own services are closed",
        );
    }
}

#[tokio::test]
async fn private_v4_literal_is_forbidden_without_dns() {
    let cfg = exit_config_with_dead_resolver();

    for literal in ["192.168.1.1", "10.0.0.1", "172.16.0.1", "100.64.0.1"] {
        let refusal = refusal_of(dial(&cfg, literal, 80).await, literal);

        then_forbidden_without_dns(
            &refusal,
            literal,
            "the exit owner's private network is closed",
        );
    }
}

#[tokio::test]
async fn closed_v6_literal_is_forbidden_without_dns() {
    let cfg = exit_config_with_dead_resolver();

    for literal in ["::1", "::ffff:192.168.1.1", "::ffff:127.0.0.1", "fe80::1"] {
        let refusal = refusal_of(dial(&cfg, literal, 80).await, literal);

        then_forbidden_without_dns(
            &refusal,
            literal,
            "a closed IPv6 literal (loopback, private network, link-local) is closed",
        );
    }
}

#[tokio::test]
async fn public_ipv6_literal_is_allowed() {
    let cfg = exit_config_with_dead_resolver();

    let addr = dial(&cfg, "2606:4700:4700::1111", 80)
        .await
        .expect("a public IPv6 literal is opened without touching the resolver");

    assert_eq!(
        addr,
        SocketAddr::from((
            "2606:4700:4700::1111"
                .parse::<std::net::Ipv6Addr>()
                .unwrap(),
            80
        ))
    );
}

#[tokio::test]
async fn mapped_ipv4_literal_becomes_ipv4() {
    let cfg = exit_config_with_dead_resolver();

    let addr = dial(&cfg, "::ffff:8.8.8.8", 53)
        .await
        .expect("a mapped IPv4 literal is judged as IPv4");

    assert_eq!(addr, SocketAddr::from(([8, 8, 8, 8], 53)));
}

#[tokio::test]
async fn trick_v4_spellings_go_to_resolver() {
    // Rust doesn't parse these as an address: they go to DoH as a name, and the "dead" resolver
    // answers with an error — no address, not a loopback that slipped through.
    let cfg = exit_config_with_dead_resolver();

    for spelling in ["0x7f.0.0.1", "2130706433", "127.1", "0177.0.0.1"] {
        let refusal = refusal_of(dial(&cfg, spelling, 80).await, spelling);

        assert_eq!(
            refusal.code, EXIT_CODE_NO_ADDRESS,
            "{spelling}: neither a literal nor a DoH name — no address"
        );
        assert!(
            refusal.line.contains("— DNS:"),
            "{spelling}: the entry went to the resolver as a name: {}",
            refusal.line
        );
    }
}

#[tokio::test]
async fn hosts_entry_bypasses_guard() {
    // given: the test table knows a "literal" with a loopback and a name with a private address
    let loopback = SocketAddr::from(([127, 0, 0, 1], 9));
    let private = SocketAddr::from(([192, 168, 1, 1], 80));
    let cfg = exit_config(HashMap::from([
        ("127.0.0.1".to_string(), loopback),
        ("site.test".to_string(), private),
    ]));

    // when: both are asked for
    let by_literal = dial(&cfg, "127.0.0.1", 80).await;
    let by_name = dial(&cfg, "site.test", 443).await;

    // then: the table value is returned as is — bypassing the closed-network table and the own-address probe
    let by_literal = by_literal.expect("the table entry bypasses the filter");
    let by_name = by_name.expect("the table entry bypasses the filter");
    assert_eq!(
        by_literal, loopback,
        "the table replaces both address and port"
    );
    assert_eq!(by_name, private, "the table replaces both address and port");
}
