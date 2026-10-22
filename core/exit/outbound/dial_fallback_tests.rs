//! Tests for `dial.rs`: IPv4 first, the AAAA fallback, several addresses one after another, the time budget.
//! A scripted connector stands in for the network; time is real, so the budgets are short.

use std::net::Ipv6Addr;
use std::time::{Duration, Instant};

use super::*;
use crate::exit::test_support::exit_config_with_silent_resolver;

const V4: &str = "93.184.216.34:443";
const V4_SECOND: &str = "1.1.1.1:443";
const V6: &str = "[2606:4700::1111]:443";

fn tight() -> Budget {
    Budget {
        total: Duration::from_millis(600),
        attempt: Duration::from_millis(150),
    }
}

fn sock(text: &str) -> SocketAddr {
    text.parse().expect("a socket address")
}

/// A config whose cache holds both families of `name` (an empty `v4` next to a non-empty `v6` is a v6-only name).
fn dual(v4: &[&str], v6: &[&str]) -> ExitConfig {
    let cfg = exit_config_with_dead_resolver();
    let v4 = v4
        .iter()
        .map(|t| t.parse::<Ipv4Addr>().expect("IPv4"))
        .collect();
    let v6 = v6
        .iter()
        .map(|t| t.parse::<Ipv6Addr>().expect("IPv6"))
        .collect();
    cfg.resolver.seed_cache_dual_for_tests("site.test", v4, v6);
    cfg
}

async fn dial_with(
    cfg: &ExitConfig,
    script: &Script,
    budget: Budget,
) -> std::result::Result<SocketAddr, Refusal> {
    open_target_with(cfg, "site.test", 443, budget, |addr| script.connect(addr)).await
}

#[tokio::test]
async fn aaaa_only_name_connects_over_ipv6() {
    let cfg = dual(&[], &["2606:4700::1111"]);
    let script = Script::all(Act::Open);

    let opened = dial_with(&cfg, &script, BUDGET)
        .await
        .expect("a v6-only name opens over IPv6");

    assert_eq!(opened, sock(V6));
    assert_eq!(script.calls(), [sock(V6)]);
}

#[tokio::test]
async fn aaaa_only_name_with_mapped_loopback_is_forbidden() {
    let cfg = dual(&[], &["::ffff:127.0.0.1"]);
    let script = Script::all(Act::Open);

    let refusal = refusal_of(dial_with(&cfg, &script, BUDGET).await, "site.test");

    assert_eq!(refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN);
    assert!(
        script.calls().is_empty(),
        "a closed address is never dialed"
    );
}

#[tokio::test]
async fn v4_name_does_not_ask_aaaa() {
    // the resolver is silent: an AAAA query would wait for its 3 s limit
    let (cfg, _silent) = exit_config_with_silent_resolver();
    cfg.resolver
        .seed_cache_for_tests("site.test", vec![Ipv4Addr::new(93, 184, 216, 34)]);
    let script = Script::all(Act::Open);
    let started = Instant::now();

    dial_with(&cfg, &script, BUDGET)
        .await
        .expect("an IPv4 name opens");

    assert!(
        started.elapsed() < Duration::from_secs(1),
        "no AAAA wait: {:?}",
        started.elapsed()
    );
    assert_eq!(script.calls(), [sock(V4)]);
}

#[tokio::test]
async fn dual_stack_prefers_ipv4() {
    let cfg = dual(&["93.184.216.34"], &["2606:4700::1111"]);
    let script = Script::all(Act::Open);

    dial_with(&cfg, &script, BUDGET).await.expect("opens");

    assert_eq!(script.calls(), [sock(V4)]);
}

#[tokio::test]
async fn refused_v4_falls_back_to_aaaa() {
    let cfg = dual(&["93.184.216.34"], &["2606:4700::1111"]);
    let script = Script::all(Act::Open).with(V4, Act::Refuse);

    let opened = dial_with(&cfg, &script, BUDGET)
        .await
        .expect("IPv6 takes over");

    assert_eq!(opened, sock(V6));
    assert_eq!(script.calls(), [sock(V4), sock(V6)]);
}

#[tokio::test]
async fn closed_v4_and_public_aaaa_connect_over_ipv6() {
    let cfg = dual(&["192.168.1.1"], &["2606:4700::1111"]);
    let script = Script::all(Act::Open);

    let opened = dial_with(&cfg, &script, BUDGET)
        .await
        .expect("the closed A is dropped, AAAA opens");

    assert_eq!(opened, sock(V6));
    assert_eq!(script.calls(), [sock(V6)]);
}

#[tokio::test]
async fn second_a_record_is_tried_after_attempt_cap() {
    let cfg = dual(&["93.184.216.34", "1.1.1.1"], &[]);
    let script = Script::all(Act::Open).with(V4, Act::Hang);
    let started = Instant::now();

    let opened = dial_with(&cfg, &script, tight())
        .await
        .expect("the second address answers");

    let took = started.elapsed();
    assert_eq!(opened, sock(V4_SECOND));
    assert!(
        took >= Duration::from_millis(150) && took < Duration::from_millis(450),
        "{took:?}"
    );
}

#[tokio::test]
async fn last_v4_takes_the_rest_of_the_budget() {
    let cfg = dual(&["93.184.216.34"], &["2606:4700::1111"]);
    let script = Script::all(Act::Open).with(V4, Act::Hang);
    let started = Instant::now();

    let refusal = refusal_of(dial_with(&cfg, &script, tight()).await, "site.test");

    let took = started.elapsed();
    assert_eq!(refusal.code, EXIT_CODE_CONNECT_FAILED);
    assert!(
        took >= Duration::from_millis(550) && took < Duration::from_millis(800),
        "{took:?}"
    );
    assert_eq!(
        script.calls(),
        [sock(V4)],
        "the budget is spent, IPv6 is not tried"
    );
}

#[tokio::test]
async fn aaaa_lookup_counts_against_the_budget() {
    // the A answer is closed, AAAA is unknown and the resolver is silent: the wait is cut to the budget
    let (cfg, _silent) = exit_config_with_silent_resolver();
    cfg.resolver
        .seed_cache_for_tests("site.test", vec![Ipv4Addr::new(10, 0, 0, 1)]);
    let script = Script::all(Act::Open);
    let budget = Budget {
        total: Duration::from_millis(400),
        attempt: Duration::from_millis(150),
    };
    let started = Instant::now();

    let refusal = refusal_of(dial_with(&cfg, &script, budget).await, "site.test");

    assert_eq!(refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN);
    assert!(
        started.elapsed() < Duration::from_millis(800),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn all_failed_is_connect_failed_and_forgets_name() {
    let cfg = dual(&["93.184.216.34"], &["2606:4700::1111"]);
    let script = Script::all(Act::Refuse);

    let refusal = refusal_of(dial_with(&cfg, &script, BUDGET).await, "site.test");

    assert_eq!(refusal.code, EXIT_CODE_CONNECT_FAILED);
    assert!(
        refusal.line.contains("2606:4700::1111"),
        "the last address tried: {}",
        refusal.line
    );
    assert!(
        refusal.line.contains("2 tried"),
        "the attempts: {}",
        refusal.line
    );
    assert_eq!(
        cfg.resolver.cached_for_tests("site.test"),
        None,
        "the stale name is forgotten"
    );
}

#[tokio::test]
async fn nonexistent_name_does_not_ask_aaaa() {
    // the cache knows an AAAA address; an NXDOMAIN answer for A must stop the exit from using it
    let cfg = dual(&[], &["2606:4700::1111"]);
    let script = Script::all(Act::Open);
    let gone = NameAnswer {
        open: vec![],
        any: false,
        nx: true,
    };

    let result = open_answers(
        &cfg,
        "site.test",
        443,
        BUDGET,
        &|addr| script.connect(addr),
        gone,
    )
    .await;

    assert_eq!(refusal_of(result, "site.test").code, EXIT_CODE_NO_ADDRESS);
    assert!(
        script.calls().is_empty(),
        "no AAAA phase for a name that does not exist"
    );
}
