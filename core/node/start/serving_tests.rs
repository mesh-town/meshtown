use super::*;
use crate::members::network_key::NetworkKey;
use crate::node::discovery_sources::tests::LOG_LOCK;
use crate::node::output::log_page;
use crate::node::start::boot::boot_endpoint;
use crate::node::start::network::spawn_network;

#[test]
fn socks_off_line_names_flag() {
    assert_eq!(socks_off_line(true), "off by the --no-socks flag");
}

#[test]
fn socks_off_line_names_settings() {
    assert_eq!(
        socks_off_line(false),
        "off in settings (socks.enabled: false)"
    );
}

#[test]
fn exit_line_names_both_modes() {
    assert_eq!(
        exit_line(true),
        format!(
            "serving as exit for members of network {}",
            crate::node::output::NET_NAME
        )
    );
    assert_eq!(exit_line(false), "not serving as exit (--no-exit)");
}

#[test]
fn blocked_line_none_when_empty() {
    assert_eq!(blocked_line(&HashSet::new()), None);
}

#[test]
fn blocked_line_sorts_names() {
    let blocked: HashSet<String> = ["b.example", "a.example"]
        .iter()
        .map(|d| d.to_string())
        .collect();
    assert_eq!(
        blocked_line(&blocked).as_deref(),
        Some("treating as blocked: a.example, b.example")
    );
}

/// Brings up the phases up to `serving` and calls `setup_serving`; returns the SOCKS5 address and the `socks` lines.
async fn serve_with(socks_enabled: bool) -> (Option<SocketAddr>, Vec<String>) {
    let _log = LOG_LOCK.lock().await;
    let mut cfg = NodeConfig::for_loopback_test(NetworkKey::generate().unwrap());
    cfg.socks.enabled = socks_enabled;
    let mut boot = boot_endpoint(&cfg).await.unwrap();
    let (net, inbound) = spawn_network(&cfg, &boot).await.unwrap();
    let access = super::super::access_wiring::access_start(&cfg, boot.id, &inbound.cache);
    let after = log_page(0, 1).last_seq;
    let serving = setup_serving(&cfg, &mut boot, &net, access).await.unwrap();
    let lines = log_page(after, 10_000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "socks")
        .map(|l| l.text)
        .collect();
    boot.background_abort_handles.iter().for_each(|t| t.abort());
    serving.socks_task.close().await;
    net.stack.close().await;
    (serving.socks_addr, lines)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn setup_serving_without_socks() {
    // given: SOCKS5 is off in settings
    let (addr, lines) = serve_with(false).await;
    // then: no address, the reason is in the log
    assert_eq!(addr, None);
    assert!(
        lines
            .iter()
            .any(|l| l == "off in settings (socks.enabled: false)"),
        "{lines:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn setup_serving_with_socks_port_zero() {
    // given: the default (port 0 — a free one)
    let (addr, _) = serve_with(true).await;
    // then: there is a listener, the port taken is a real one
    assert_ne!(addr.expect("SOCKS5 address").port(), 0);
}
