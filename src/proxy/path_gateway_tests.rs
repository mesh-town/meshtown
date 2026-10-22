//! Tests for the UDP gateway memory in `open_through_exits` (`path.rs`) against an exit that scripts its reply.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::busy_tests::config_with;
use super::*;
use crate::exit::test_support::Scripted;
use crate::net::facade::test_support::{pid, within};
use crate::net::ServiceHandler;

const FORBIDDEN: &[u8] = &[exit::EXIT_PROTOCOL_VERSION, exit::EXIT_CODE_ADDRESS_FORBIDDEN];

/// A config whose exits answer `FORBIDDEN`, and the request counter of exit 2.
async fn forbidding() -> (ProxyConfig, Arc<std::sync::atomic::AtomicUsize>) {
    let scripted = Arc::new(Scripted::new(FORBIDDEN));
    let hits = scripted.hits.clone();
    let cfg = config_with(&[], || scripted.clone() as Arc<dyn ServiceHandler>).await;
    (cfg, hits)
}

async fn open(cfg: &ProxyConfig, host: &str, port: u16) -> OpenRefused {
    let res = within("open_through_exits", open_through_exits(cfg, &[pid(2)], host, port)).await;
    res.err().expect("the exit refuses")
}

/// given: an exit that answers code 7; when the gateway target is requested twice, then the exit sees one
/// request and both refusals carry the same text.
#[tokio::test]
async fn real_code_7_on_the_gateway_target_is_remembered() {
    let (cfg, hits) = forbidding().await;

    let first = open(&cfg, exit::UDPGW_HOST, exit::UDPGW_PORT).await;
    let second = open(&cfg, exit::UDPGW_HOST, exit::UDPGW_PORT).await;

    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(cfg.pool.gateway_refused_recently(pid(2)));
    assert_eq!(first.outcome.result, second.outcome.result);
    assert!(second.outcome.result.contains("code 7"), "{}", second.outcome.result);
}

/// given: the gateway refusal is remembered; when an ordinary target is requested, then the exit is asked.
#[tokio::test]
async fn ordinary_target_still_reaches_the_exit() {
    let (cfg, hits) = forbidding().await;
    open(&cfg, exit::UDPGW_HOST, exit::UDPGW_PORT).await;

    open(&cfg, "example.com", 443).await;

    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

/// given: an ordinary target refused with code 7; then nothing is remembered for the gateway.
#[tokio::test]
async fn code_7_on_an_ordinary_target_is_not_remembered() {
    let (cfg, _hits) = forbidding().await;

    open(&cfg, "example.com", 443).await;

    assert!(!cfg.pool.gateway_refused_recently(pid(2)));
}
