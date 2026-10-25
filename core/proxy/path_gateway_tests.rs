//! Tests for the UDP gateway memory in `open_through_exits` (`path.rs`) against an exit that scripts its reply.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::busy_tests::config_with;
use super::*;
use crate::exit::test_support::Scripted;
use crate::transport::net::facade::test_support::{pid, within};
use crate::transport::net::ServiceHandler;

const FORBIDDEN: &[u8] = &[
    exit::EXIT_PROTOCOL_VERSION,
    exit::EXIT_CODE_ADDRESS_FORBIDDEN,
];

/// A config whose exits answer `FORBIDDEN`, and the request counter of exit 2.
async fn forbidding() -> (ProxyConfig, Arc<std::sync::atomic::AtomicUsize>) {
    let scripted = Arc::new(Scripted::new(FORBIDDEN));
    let hits = scripted.hits.clone();
    let cfg = config_with(&[], || scripted.clone() as Arc<dyn ServiceHandler>).await;
    (cfg, hits)
}

async fn open(cfg: &ProxyConfig, host: &str, port: u16) -> OpenRefused {
    let res = within(
        "open_through_exits",
        open_through_exits(cfg, &[pid(2)], host, port),
    )
    .await;
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
    assert!(
        second.outcome.result.contains("code 7"),
        "{}",
        second.outcome.result
    );
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

const OK: &[u8] = &[exit::EXIT_PROTOCOL_VERSION, exit::EXIT_CODE_OK];

/// A config whose exit 2 answers `first` and exit 3 answers `second`, with their request counters.
async fn two_exits(
    first: &'static [u8],
    second: &'static [u8],
    stuck: &[u8],
) -> (
    ProxyConfig,
    Arc<std::sync::atomic::AtomicUsize>,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let (x, y) = (
        Arc::new(Scripted::new(first)),
        Arc::new(Scripted::new(second)),
    );
    let (x_hits, y_hits) = (x.hits.clone(), y.hits.clone());
    let queue = std::sync::Mutex::new(vec![
        y as Arc<dyn ServiceHandler>,
        x as Arc<dyn ServiceHandler>,
    ]);
    let cfg = config_with(stuck, || queue.lock().unwrap().pop().expect("two exits")).await;
    (cfg, x_hits, y_hits)
}

async fn open_both(cfg: &ProxyConfig, host: &str, port: u16) -> Result<ExitLink, OpenRefused> {
    within(
        "open_through_exits",
        open_through_exits(cfg, &[pid(2), pid(3)], host, port),
    )
    .await
}

/// given: a config without chosen exits; when the UDP route is opened, then it is direct and no exit is asked.
#[tokio::test]
async fn udp_route_on_direct_is_direct() {
    let (cfg, hits) = forbidding().await;

    let route = within("open_udp_route", open_udp_route(&cfg)).await;

    assert!(matches!(route, Ok(UdpRoute::Direct)));
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

/// given: an exit that answers code 7; when the exit link is opened twice, then both refusals say "no UDP"
/// and the exit sees one request.
#[tokio::test]
async fn udp_route_refused_by_exit_is_no_udp_and_remembered() {
    let (cfg, hits) = forbidding().await;

    for _ in 0..2 {
        let refusal = within("open_exit_link", udp::open_exit_link(&cfg, &[pid(2)]))
            .await
            .err()
            .expect("refused");
        assert!(refusal.no_udp);
    }

    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

/// given: an exit with a gateway; when the exit link is opened, then it is an exit link.
#[tokio::test]
async fn udp_route_ok_gives_exit_link() {
    let (cfg, _x, _y) = two_exits(OK, OK, &[]).await;

    let link = within("open_exit_link", udp::open_exit_link(&cfg, &[pid(2)]))
        .await
        .ok()
        .expect("opened");

    assert_eq!(link.peer, pid(2));
}

/// given: exits [X: code 7, Y: OK]; when the gateway target is requested, then the stream goes to Y.
#[tokio::test]
async fn gateway_refusal_moves_to_next_exit() {
    let (cfg, x, y) = two_exits(FORBIDDEN, OK, &[]).await;

    let opened = open_both(&cfg, exit::UDPGW_HOST, exit::UDPGW_PORT)
        .await
        .ok()
        .expect("Y answers");

    assert_eq!(
        opened.route_label,
        format!("exit {}", short_peer_id(&pid(3)))
    );
    assert_eq!((x.load(Ordering::SeqCst), y.load(Ordering::SeqCst)), (1, 1));
}

/// given: X is remembered as refusing the gateway; when requested again, then X is not asked and Y answers.
#[tokio::test]
async fn remembered_refusal_skips_to_next_exit_without_asking() {
    let (cfg, x, y) = two_exits(FORBIDDEN, OK, &[]).await;
    open_both(&cfg, exit::UDPGW_HOST, exit::UDPGW_PORT)
        .await
        .ok()
        .expect("Y answers");

    let opened = open_both(&cfg, exit::UDPGW_HOST, exit::UDPGW_PORT)
        .await
        .ok()
        .expect("Y answers");

    assert_eq!(
        opened.route_label,
        format!("exit {}", short_peer_id(&pid(3)))
    );
    assert_eq!((x.load(Ordering::SeqCst), y.load(Ordering::SeqCst)), (1, 2));
}

/// given: both exits answer code 7; then the refusal is "no UDP gateway".
#[tokio::test]
async fn all_exits_refusing_gateway_is_no_udp() {
    let (cfg, _x, _y) = two_exits(FORBIDDEN, FORBIDDEN, &[]).await;

    let refused = open_both(&cfg, exit::UDPGW_HOST, exit::UDPGW_PORT)
        .await
        .err()
        .expect("refused");

    assert!(refused.gateway_refused);
}

/// given: an ordinary target and X answers code 7; then the refusal is final and Y is not asked.
#[tokio::test]
async fn ordinary_target_refusal_stays_final() {
    let (cfg, _x, y) = two_exits(FORBIDDEN, OK, &[]).await;

    let refused = open_both(&cfg, "example.com", 443)
        .await
        .err()
        .expect("refused");

    assert!(!refused.gateway_refused);
    assert_eq!(y.load(Ordering::SeqCst), 0);
}

const CONNECT_FAILED: &[u8] = &[exit::EXIT_PROTOCOL_VERSION, exit::EXIT_CODE_CONNECT_FAILED];

/// Opens the gateway target through both exits and returns the refusal.
async fn gateway_refusal(cfg: &ProxyConfig) -> OpenRefused {
    open_both(cfg, exit::UDPGW_HOST, exit::UDPGW_PORT)
        .await
        .err()
        .expect("refused")
}

/// given: X answers code 7, Y does not answer; then the refusal is not "no UDP gateway".
#[tokio::test]
async fn code_7_then_silent_exit_is_not_no_udp() {
    let (cfg, _x, _y) = two_exits(FORBIDDEN, OK, &[3]).await;

    let refused = gateway_refusal(&cfg).await;

    assert!(!refused.gateway_refused);
    assert!(refused.outcome.result.contains("exit is busy"));
}

/// given: X does not answer, Y answers code 7; then the refusal is not "no UDP gateway".
#[tokio::test]
async fn silent_then_code_7_is_not_no_udp() {
    let (cfg, _x, _y) = two_exits(OK, FORBIDDEN, &[2]).await;

    let refused = gateway_refusal(&cfg).await;

    assert!(!refused.gateway_refused);
}

/// given: X is remembered as refusing the gateway, Y does not answer; then the refusal is not "no UDP gateway".
#[tokio::test]
async fn remembered_refusal_then_silent_exit_is_not_no_udp() {
    let (cfg, _x, _y) = two_exits(OK, OK, &[3]).await;
    cfg.pool.note_gateway_refused(pid(2));

    let refused = gateway_refusal(&cfg).await;

    assert!(!refused.gateway_refused);
}

/// given: X is remembered as refusing the gateway, Y answers code 7; then the refusal is "no UDP gateway".
#[tokio::test]
async fn remembered_and_fresh_code_7_is_no_udp() {
    let (cfg, _x, _y) = two_exits(OK, FORBIDDEN, &[]).await;
    cfg.pool.note_gateway_refused(pid(2));

    let refused = gateway_refusal(&cfg).await;

    assert!(refused.gateway_refused);
}

/// given: X answers another error code, Y is fine; then the refusal is final and Y is not asked.
#[tokio::test]
async fn other_code_on_gateway_target_is_final() {
    let (cfg, x, y) = two_exits(CONNECT_FAILED, OK, &[]).await;

    let refused = gateway_refusal(&cfg).await;

    assert!(!refused.gateway_refused);
    assert_eq!((x.load(Ordering::SeqCst), y.load(Ordering::SeqCst)), (1, 0));
}

/// given: no candidates; then the refusal is "no route", not "no UDP gateway".
#[tokio::test]
async fn no_candidates_is_not_no_udp() {
    let (cfg, _x, _y) = two_exits(OK, OK, &[]).await;

    let res = within(
        "open_through_exits",
        open_through_exits(&cfg, &[], exit::UDPGW_HOST, exit::UDPGW_PORT),
    )
    .await;

    let refused = res.err().expect("nothing to try");
    assert!(!refused.gateway_refused);
    assert_eq!(refused.outcome.route_label, "no route");
}

/// given: X answers code 7, Y does not answer; then the UDP refusal is not "no UDP".
#[tokio::test]
async fn udp_link_mixed_refusal_is_not_no_udp() {
    let (cfg, _x, _y) = two_exits(FORBIDDEN, OK, &[3]).await;

    let refusal = within(
        "open_exit_link",
        udp::open_exit_link(&cfg, &[pid(2), pid(3)]),
    )
    .await
    .err()
    .expect("refused");

    assert!(!refusal.no_udp);
}

/// given: the chosen exit is not in the network; then the UDP route is refused by policy, not as "no UDP".
#[tokio::test]
async fn udp_route_refused_by_policy_is_not_no_udp() {
    let (cfg, _x, _y) = two_exits(OK, OK, &[]).await;
    cfg.route.set_chosen_exits(vec![pid(9)]);

    let refusal = within("open_udp_route", open_udp_route(&cfg))
        .await
        .err()
        .expect("refused");

    assert!(!refusal.no_udp);
}
