//! Integration tests of the check through a member's exit: A checks a domain through B's exit and gets a record.
//!
//! What it does: two real nodes on loopback (`common::pair`); behind B's exit (`exit_hosts`) the name `DOMAIN`
//! leads to a local TLS server with a throwaway self-signed certificate, which A trusts through
//! `CheckConfig::extra_roots`. Checks that the record is `ok` with the bytes counted, that an exit that is
//! switched off gives no result, and that `Checks::check_via_exits` keeps a result for an exit that is on.
//! What it doesn't do: doesn't go to the internet, doesn't check the schedule (`checks/schedule_tests.rs`) or the
//! snapshot column (`node_ctl/snapshot/build_tests.rs`).
//! Links: `meshtown_core::site_checks::reachability::check_domain_via_exit`, `Checks::check_via_exits`;
//! plumbing — `core/tests/common/socks_pair.rs`.

#[path = "common/socks_pair.rs"]
mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use meshtown_core::exit::PeerConnPool;
use meshtown_core::node::node_state;
use meshtown_core::node::RunningNode;
use meshtown_core::site_checks::reachability::{
    check_domain_via_exit, classify, CheckConfig, CheckVerdict, Steps, TcpStep,
};
use meshtown_core::transport::net::PeerId;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{sleep, Instant};
use tokio_rustls::TlsAcceptor;

use common::{pair, DOMAIN};

/// A throwaway self-signed certificate for `site.test` (EC P-256, no CA flag), base64 of the DER.
const CERT_B64: &str = "MIIBajCCAQ+gAwIBAgIJAIpFFYY+0pP+MAoGCCqGSM49BAMCMBQxEjAQBgNVBAMMCXNpdGUudGVzdDAgFw0yNjExMDYxOTE0NDhaGA8yMTI2MTAxMzE5MTQ0OFowFDESMBAGA1UEAwwJc2l0ZS50ZXN0MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE6JXXdys98jx6wbbHwONcZwbQXdLL0yeZ4NSdNW/39WRfSO/3SZBpMDMyPdgdHjLXzPTqxIdwO9BwFDUognqrvaNIMEYwFAYDVR0RBA0wC4IJc2l0ZS50ZXN0MAwGA1UdEwEB/wQCMAAwCwYDVR0PBAQDAgeAMBMGA1UdJQQMMAoGCCsGAQUFBwMBMAoGCCqGSM49BAMCA0kAMEYCIQDJ4WtMa2XXj3w6s3vd3Kiym1XAwrC9PlhTuVoZe6PyKAIhAICEZrDX2/A4dnQz4dPM0COklALFY7Ryu/S25qjWOhFj";
/// Its private key (PKCS#8), base64 of the DER. Only for these tests.
const KEY_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgjipw2CpnMR1SOJwMR1+9RoB8cl00lTgdGLkMp/xphUKhRANCAAToldd3Kz3yPHrBtsfA41xnBtBd0svTJ5ng1J01b/f1ZF9I7/dJkGkwMzI92B0eMtfM9OrEh3A70HAUNSiCequ9";

const BODY_BYTES: usize = 4096;
const FLAG_WITHIN: Duration = Duration::from_secs(20);

fn cert_der() -> Vec<u8> {
    data_encoding::BASE64.decode(CERT_B64.as_bytes()).unwrap()
}

/// A TLS server that answers any request with 200 and `BODY_BYTES` bytes of body.
async fn tls_site() -> SocketAddr {
    let key = PrivatePkcs8KeyDer::from(data_encoding::BASE64.decode(KEY_B64.as_bytes()).unwrap());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(cert_der())],
            PrivateKeyDer::from(key),
        )
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    meshtown_core::node::output::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            meshtown_core::node::output::spawn(async move { answer(acceptor, tcp).await });
        }
    });
    addr
}

async fn answer(acceptor: TlsAcceptor, tcp: tokio::net::TcpStream) {
    let Ok(mut tls) = acceptor.accept(tcp).await else {
        return;
    };
    let mut buf = vec![0u8; 2048];
    let _ = tls.read(&mut buf).await;
    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {BODY_BYTES}\r\n\r\n");
    let _ = tls.write_all(head.as_bytes()).await;
    let _ = tls.write_all(&vec![b'x'; BODY_BYTES]).await;
    let _ = tls.shutdown().await;
}

/// Waits until A's copy of the shared document carries B's exit flag with the wanted value.
async fn wait_exit_flag(a: &RunningNode, b: PeerId, want: bool) {
    let deadline = Instant::now() + FLAG_WITHIN;
    loop {
        let flags = node_state::read_exit_flags(&*a.domains.shared_replica())
            .await
            .unwrap();
        if flags.get(&b) == Some(&want) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "B's exit flag {want} did not reach A"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_via_exit_reaches_local_server() {
    // given: A and B (exit on); behind B's exit `site.test` is a local TLS server that A trusts
    let (a, b) = pair(tls_site().await, true).await;
    wait_exit_flag(&a, b.id, true).await;
    let config = CheckConfig {
        extra_roots: Arc::from(vec![cert_der()]),
        ..CheckConfig::default()
    };

    // when: A checks the domain through B
    let pool = PeerConnPool::default();
    let record = check_domain_via_exit(&config, &a.stack, &pool, b.id, DOMAIN)
        .await
        .expect("the exit must answer");

    // then: the page opened and the bytes were counted
    assert_eq!(
        record.verdict,
        CheckVerdict::Ok,
        "reason: {}",
        record.reason
    );
    assert_eq!(record.status, 200);
    assert!(
        record.bytes as usize >= BODY_BYTES,
        "bytes: {}",
        record.bytes
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_via_non_exit_peer_is_skipped() {
    // given: B has its exit switched off and A already knows it
    let (a, b) = pair(tls_site().await, false).await;
    wait_exit_flag(&a, b.id, false).await;

    // when: A checks the domain through all current exits
    let stored = a.checks.check_via_exits(DOMAIN).await;

    // then: nothing was asked of B and no result exists
    assert_eq!(stored, 0);
    assert!(a.checks.exit_results().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_via_exits_keeps_result_per_exit() {
    // given: B's exit is on (A does not trust the test certificate here, so the verdict is not `ok`)
    let (a, b) = pair(tls_site().await, true).await;
    wait_exit_flag(&a, b.id, true).await;

    // when
    let stored = a.checks.check_via_exits(DOMAIN).await;

    // then: a result under (domain, B)
    assert_eq!(stored, 1);
    let results = a.checks.exit_results();
    assert!(results.contains_key(&(DOMAIN.to_string(), b.id)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_via_exit_connect_failed_gives_tcp_timeout() {
    // given: the site address is a closed port, so B's exit cannot connect
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap()
    };
    let (a, b) = pair(closed, true).await;
    wait_exit_flag(&a, b.id, true).await;

    // when
    let pool = PeerConnPool::default();
    let record = check_domain_via_exit(&CheckConfig::default(), &a.stack, &pool, b.id, DOMAIN)
        .await
        .expect("the exit answered with a code that is a result");

    // then: a TCP timeout step, no TLS step
    let steps = Steps {
        dns_ok: true,
        tcp: TcpStep::Timeout,
        ..Steps::default()
    };
    assert_eq!(record.tls_ms, 0);
    assert_eq!(record.status, 0);
    assert_eq!((record.verdict, record.reason), classify(&steps));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn block_site_flag_does_not_apply_through_exit() {
    // given: as in the first test, but the domain is blocked for this node
    let (a, b) = pair(tls_site().await, true).await;
    wait_exit_flag(&a, b.id, true).await;
    let config = CheckConfig {
        extra_roots: Arc::from(vec![cert_der()]),
        blocked_domains: Arc::new([DOMAIN.to_string()].into_iter().collect()),
        ..CheckConfig::default()
    };

    // when
    let pool = PeerConnPool::default();
    let record = check_domain_via_exit(&config, &a.stack, &pool, b.id, DOMAIN)
        .await
        .expect("the exit must answer");

    // then: the exit's network sees the site, so the verdict is ok
    assert_eq!(
        record.verdict,
        CheckVerdict::Ok,
        "reason: {}",
        record.reason
    );
}
