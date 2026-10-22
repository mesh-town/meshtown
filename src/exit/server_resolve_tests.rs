//! Tests for `exit/server.rs`: parsing the destination address (`resolve_address`).

use super::*;

#[tokio::test]
async fn ipv4_literal_skips_dns() {
    let cfg = exit_config_with_dead_resolver();

    let addr = resolve_address(&cfg, "1.2.3.4", 443)
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
        let refusal = refusal_of(resolve_address(&cfg, literal, 80).await, literal);

        // then: refusal with code 7 without going to DNS
        then_forbidden_without_dns(&refusal, literal, "the loopback — the exit's own services are closed");
    }
}

#[tokio::test]
async fn private_v4_literal_is_forbidden_without_dns() {
    let cfg = exit_config_with_dead_resolver();

    for literal in ["192.168.1.1", "10.0.0.1", "172.16.0.1", "100.64.0.1"] {
        let refusal = refusal_of(resolve_address(&cfg, literal, 80).await, literal);

        then_forbidden_without_dns(&refusal, literal, "the exit owner's private network is closed");
    }
}

#[tokio::test]
async fn closed_v6_literal_is_forbidden_without_dns() {
    let cfg = exit_config_with_dead_resolver();

    for literal in ["::1", "::ffff:192.168.1.1", "::ffff:127.0.0.1", "fe80::1"] {
        let refusal = refusal_of(resolve_address(&cfg, literal, 80).await, literal);

        then_forbidden_without_dns(
            &refusal,
            literal,
            "a closed IPv6 literal (loopback, private network, link-local) is closed",
        );
    }
}

#[tokio::test]
async fn open_v6_literal_is_no_address_without_dns() {
    let cfg = exit_config_with_dead_resolver();

    for literal in ["2606:4700:4700::1111", "::ffff:8.8.8.8"] {
        let refusal = refusal_of(resolve_address(&cfg, literal, 80).await, literal);

        assert_eq!(
            refusal.code, EXIT_CODE_NO_ADDRESS,
            "{literal}: the exit doesn't open a public IPv6 literal — no address, not \"closed\""
        );
        assert!(
            refusal.line.contains("IPv6"),
            "{literal}: the reason is named in the log line: {}",
            refusal.line
        );
        assert!(
            !refusal.line.contains("DNS"),
            "{literal}: an address string doesn't go to DoH: {}",
            refusal.line
        );
    }
}

#[tokio::test]
async fn trick_v4_spellings_go_to_resolver() {
    // Rust doesn't parse these as an address: they go to DoH as a name, and the "dead" resolver
    // answers with an error — no address, not a loopback that slipped through.
    let cfg = exit_config_with_dead_resolver();

    for spelling in ["0x7f.0.0.1", "2130706433", "127.1", "0177.0.0.1"] {
        let refusal = refusal_of(resolve_address(&cfg, spelling, 80).await, spelling);

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
    let by_literal = resolve_address(&cfg, "127.0.0.1", 80).await;
    let by_name = resolve_address(&cfg, "site.test", 443).await;

    // then: the table value is returned as is — bypassing the closed-network table and the own-address probe
    let by_literal = by_literal.expect("the table entry bypasses the filter");
    let by_name = by_name.expect("the table entry bypasses the filter");
    assert_eq!(by_literal, loopback, "the table replaces both address and port");
    assert_eq!(by_name, private, "the table replaces both address and port");
}

async fn resolve_named(
    domain: &str,
    addrs: &[[u8; 4]],
    port: u16,
) -> std::result::Result<SocketAddr, Refusal> {
    let cfg = exit_config_with_dead_resolver();
    let addrs = addrs.iter().map(|a| Ipv4Addr::from(*a)).collect();
    cfg.resolver.seed_cache_for_tests(domain, addrs);
    resolve_address(&cfg, domain, port).await
}

fn then_name_forbidden(refusal: &Refusal, domain: &str, hidden: &str) {
    assert_eq!(
        refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
        "{domain}: a closed address in the DoH answer — code 7"
    );
    assert!(
        refusal.line.contains(domain),
        "{domain}: the log line names the host: {}",
        refusal.line
    );
    assert!(
        !refusal.line.contains(hidden),
        "{domain}: the address from DoH is not given out: {}",
        refusal.line
    );
}

#[tokio::test]
async fn name_with_private_a_record_is_forbidden() {
    let result = resolve_named("nas.example.test", &[[192, 168, 1, 1]], 443).await;
    then_name_forbidden(&refusal_of(result, "nas"), "nas.example.test", "192.168");
}

#[tokio::test]
async fn name_with_10_a_record_is_forbidden() {
    let result = resolve_named("lan.example.test", &[[10, 0, 0, 5]], 443).await;
    then_name_forbidden(&refusal_of(result, "lan"), "lan.example.test", "10.0.0.5");
}

#[tokio::test]
async fn name_with_loopback_a_record_is_forbidden() {
    let result = resolve_named("localtest.example.test", &[[127, 0, 0, 1]], 80).await;
    then_name_forbidden(
        &refusal_of(result, "lo"),
        "localtest.example.test",
        "127.0.0.1",
    );
}

#[tokio::test]
async fn name_with_mixed_a_records_takes_the_public_one() {
    let result = resolve_named(
        "mixed.example.test",
        &[[10, 0, 0, 1], [93, 184, 216, 34]],
        443,
    )
    .await;

    let addr = result.expect("there is an open address among them — it gets picked");
    assert_eq!(addr, SocketAddr::from(([93, 184, 216, 34], 443)));
}

#[tokio::test]
async fn name_with_public_a_record_resolves_from_cache() {
    let result = resolve_named("site.example.test", &[[93, 184, 216, 34]], 8443);
    let addr = result.await.expect("an open address from the cache without network");
    assert_eq!(addr, SocketAddr::from(([93, 184, 216, 34], 8443)));
}
