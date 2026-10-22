//! Tests for `dial.rs`: names answered from the resolver cache (`resolve_address`).

use super::*;

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
