//! A loopback DNS-over-HTTPS server for tests: real TLS with a fixed certificate for `127.0.0.1`, HTTP/1.1, and
//! canned DNS answers chosen by the name in the query.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use data_encoding::BASE64URL_NOPAD;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use simple_dns::rdata::{RData, A};
use simple_dns::{Packet, PacketFlag, ResourceRecord, CLASS, QTYPE, RCODE, TYPE};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use super::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::task_label::spawn;

const CERT_PEM: &[u8] = include_bytes!("testdata/doh_test_cert.pem");
/// PKCS#8 key of the test certificate, base64 without armor lines.
const KEY_B64: &str = include_str!("testdata/doh_test_key.b64");

/// The address `host.test` names resolve to (when they are not special).
pub(crate) const ANSWER_V4: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 7);

/// A running test server. Names ending in `nx.test` get NXDOMAIN, names ending in `empty.test` an empty successful
/// answer, any other name `ANSWER_V4` for A and an empty answer for AAAA.
pub(crate) struct TestDoh {
    pub addr: SocketAddr,
    queries: Arc<Mutex<Vec<(String, TYPE)>>>,
}

impl TestDoh {
    /// Starts the server on a free loopback port; it lives as long as the runtime of the test.
    pub(crate) async fn start() -> TestDoh {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local address");
        let queries = Arc::new(Mutex::new(Vec::new()));
        let (acceptor, log) = (acceptor(), queries.clone());
        spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                spawn(serve(acceptor.clone(), tcp, log.clone()));
            }
        });
        TestDoh { addr, queries }
    }

    /// A resolver that trusts this server's certificate.
    pub(crate) fn resolver(&self) -> Resolver {
        let limits = HttpLimits::default();
        Resolver::with_tls_for_tests(self.addr, limits, "meshtown-test".into(), client_config())
    }

    /// Every query received so far: the name and the asked record type.
    pub(crate) fn queries(&self) -> Vec<(String, TYPE)> {
        self.queries.lock().expect("query log").clone()
    }
}

fn certificate() -> CertificateDer<'static> {
    CertificateDer::from_pem_slice(CERT_PEM).expect("test certificate")
}

fn acceptor() -> TlsAcceptor {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let body: String = KEY_B64.split_whitespace().collect();
    let der = data_encoding::BASE64
        .decode(body.as_bytes())
        .expect("test key");
    let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(der));
    let mut cfg = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_no_client_auth()
        .with_single_cert(vec![certificate()], key)
        .expect("server certificate");
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    TlsAcceptor::from(Arc::new(cfg))
}

/// A client config whose only trust anchor is the test certificate.
fn client_config() -> Arc<ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.add(certificate()).expect("trust anchor");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(cfg)
}

async fn serve(
    acceptor: TlsAcceptor,
    tcp: tokio::net::TcpStream,
    log: Arc<Mutex<Vec<(String, TYPE)>>>,
) {
    let Ok(mut tls) = acceptor.accept(tcp).await else {
        return;
    };
    let mut head = Vec::new();
    let mut chunk = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match tls.read(&mut chunk).await {
            Ok(n) if n > 0 => head.extend_from_slice(&chunk[..n]),
            _ => return,
        }
    }
    let Some(query) = query_of(&head) else { return };
    let packet = Packet::parse(&query).expect("DNS query");
    let question = &packet.questions[0];
    let (name, qtype) = (question.qname.to_string(), qtype_of(&question.qtype));
    log.lock().expect("query log").push((name.clone(), qtype));
    let body = reply(&name, qtype);
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = tls.write_all(head.as_bytes()).await;
    let _ = tls.write_all(&body).await;
    let _ = tls.shutdown().await;
}

/// The DNS message from `GET /dns-query?dns=<base64url>`.
fn query_of(head: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(head).ok()?;
    let target = text.lines().next()?.split(' ').nth(1)?;
    let encoded = target.split_once("dns=")?.1;
    BASE64URL_NOPAD.decode(encoded.as_bytes()).ok()
}

fn qtype_of(qtype: &QTYPE) -> TYPE {
    match qtype {
        QTYPE::TYPE(t) => *t,
        _ => TYPE::A,
    }
}

fn reply(name: &str, qtype: TYPE) -> Vec<u8> {
    let mut packet = Packet::new_query(0);
    packet.set_flags(PacketFlag::RESPONSE);
    if name.trim_end_matches('.').ends_with("nx.test") {
        *packet.rcode_mut() = RCODE::NameError;
    } else if qtype == TYPE::A && !name.trim_end_matches('.').ends_with("empty.test") {
        let address = u32::from(ANSWER_V4);
        packet.answers.push(ResourceRecord::new(
            simple_dns::Name::new(name).expect("name"),
            CLASS::IN,
            60,
            RData::A(A { address }),
        ));
    }
    packet.build_bytes_vec().expect("DNS answer")
}
