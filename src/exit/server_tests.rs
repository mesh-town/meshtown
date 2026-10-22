//! Tests for `exit/server.rs`: the domain address without a DNS query for an IPv4 literal
//! (a CONNECT to a bare IP goes to the exit directly) and refusal for
//! closed addresses (`address_guard`): literals, the DoH answer, the node's own address, connection
//! failure. The tests need no network: the resolver is "dead", port 1 on the loopback isn't listening.

use std::net::UdpSocket;

use super::*;
use crate::http_min::HttpLimits;

#[path = "server_resolve_tests.rs"]
mod resolve_address;

mod named_address {
    use super::*;

    #[test]
    fn all_closed_answer_is_forbidden_without_address_in_line() {
        let answer = [Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(127, 0, 0, 1)];

        let refusal = refusal_of(
            first_allowed_resolved_address(&answer, "router.example", 443),
            "router.example",
        );

        assert_eq!(
            refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
            "all addresses in the answer are closed — refusal with code 7"
        );
        assert!(
            refusal.line.contains("router.example"),
            "the log line names the host: {}",
            refusal.line
        );
        assert!(
            !refusal.line.contains("192.168") && !refusal.line.contains("127.0.0.1"),
            "the address from the DoH answer is not given out in the log: {}",
            refusal.line
        );
    }

    #[test]
    fn mixed_answer_picks_first_open() {
        let answer = [
            Ipv4Addr::new(192, 168, 1, 1),
            Ipv4Addr::new(93, 184, 216, 34),
        ];

        let addr = first_allowed_resolved_address(&answer, "mixed.example", 443)
            .expect("there is an open address among them — it gets chosen");

        assert_eq!(addr, SocketAddr::from(([93, 184, 216, 34], 443)));
    }

    #[test]
    fn empty_answer_is_no_address() {
        let refusal = refusal_of(first_allowed_resolved_address(&[], "empty.example", 443), "empty.example");

        assert_eq!(
            refusal.code, EXIT_CODE_NO_ADDRESS,
            "an empty DoH answer gives \"no usable address\", not \"closed\""
        );
    }
}

mod not_own {
    use super::*;

    #[test]
    fn own_address_is_forbidden_before_connect() {
        // the loopback is the node's own address in any environment
        let loopback = SocketAddr::from(([127, 0, 0, 1], 80));
        let refusal = refusal_of(reject_own_address(loopback, "self.test"), "127.0.0.1:80");
        assert_eq!(
            refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
            "the loopback is the node's own address, refusal with code 7"
        );

        // the address the host uses to go outside is also its own (no route — the probe is skipped)
        let Some(src) = outgoing_source() else {
            eprintln!("no default route — the outgoing address probe is skipped");
            return;
        };
        let refusal = refusal_of(
            reject_own_address(SocketAddr::new(src, 80), "own.test"),
            "outgoing address",
        );
        assert_eq!(
            refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
            "{src}: the node's outgoing address is its own, refusal with code 7"
        );
        assert!(
            refusal.line.contains("own.test"),
            "the log line names the host: {}",
            refusal.line
        );
        assert!(
            !refusal.line.contains(&src.to_string()),
            "the own address is not given out in the log: {}",
            refusal.line
        );
    }

    #[test]
    fn foreign_address_passes_own_check() {
        let foreign = SocketAddr::from(([192, 0, 2, 1], 80));

        let passed = reject_own_address(foreign, "x").expect("a foreign address passes the own-address check");

        assert_eq!(passed, foreign, "the address is returned unchanged");
    }
}

mod open_tcp {
    use super::*;

    #[tokio::test]
    async fn closed_port_is_connect_failed() {
        let cfg = exit_config_with_dead_resolver();
        let closed = SocketAddr::from(([127, 0, 0, 1], 1));

        let refusal = refusal_of(open_tcp(&cfg, "closed.test", closed).await, "127.0.0.1:1");

        assert_eq!(
            refusal.code, EXIT_CODE_CONNECT_FAILED,
            "a closed port is a connection failure, not \"closed\""
        );
        assert!(
            refusal.line.contains("connection didn't open"),
            "the log line names the reason: {}",
            refusal.line
        );
    }
}

mod low_latency {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn low_latency_sets_nodelay() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let tcp = TcpStream::connect(addr).await.unwrap();
        let (_accepted, _) = listener.accept().await.unwrap();
        assert!(!tcp.nodelay().unwrap(), "Nagle is on by default");

        disable_nagle(&tcp);

        assert!(tcp.nodelay().unwrap());
    }
}

/// `ExitConfig` with a resolver pointing at a closed loopback port: an ordinary domain name
/// fails on trying to reach it rather than quietly getting an answer — this is how the test catches an accidental
/// DNS query for an IP literal.
fn exit_config_with_dead_resolver() -> ExitConfig {
    exit_config(HashMap::new())
}

/// The same with the test table `host_overrides`: entries bypass the address filter and the own-address probe.
fn exit_config(hosts: HashMap<String, SocketAddr>) -> ExitConfig {
    let resolver = Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a stub address");

    ExitConfig {
        enabled: Arc::new(AtomicBool::new(true)),
        members: Members::default(),
        list: Arc::new(OnceLock::new()),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(hosts),
        clients: Clients::default(),
        traffic: ExitTraffic::default(),
    }
}

/// A refusal from the result; success panics with the name of what was asked.
fn refusal_of<T>(result: std::result::Result<T, Refusal>, what: &str) -> Refusal {
    match result {
        Ok(_) => panic!("{what}: a refusal was expected, but an address was given"),
        Err(refusal) => refusal,
    }
}

/// Refusal for a closed literal: code 7, the log line has the literal and no word "DNS".
fn then_forbidden_without_dns(refusal: &Refusal, literal: &str, why: &str) {
    assert_eq!(
        refusal.code, EXIT_CODE_ADDRESS_FORBIDDEN,
        "{literal}: {why} (code 7)"
    );
    assert!(
        refusal.line.contains(literal),
        "{literal}: the log line names the literal: {}",
        refusal.line
    );
    assert!(
        !refusal.line.contains("DNS"),
        "{literal}: the literal does not go to DNS: {}",
        refusal.line
    );
}

/// The address this host uses to go outside: a UDP `connect` only picks a route and sends
/// no packets. An error (no default route) gives `None`.
fn outgoing_source() -> Option<IpAddr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    sock.local_addr().ok().map(|addr| addr.ip())
}
