//! Tests for `dial.rs`: refusal for closed addresses and connection failure. The tests need no network:
//! the resolver is "dead", port 1 on the loopback isn't listening.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;

use super::*;
use crate::exit::frame::{
    EXIT_CODE_ADDRESS_FORBIDDEN, EXIT_CODE_CONNECT_FAILED, EXIT_CODE_NO_ADDRESS,
};
use crate::exit::test_support::{exit_config, exit_config_with_dead_resolver, refusal_of};

#[path = "dial_fallback_tests.rs"]
mod fallback;
#[path = "dial_literal_tests.rs"]
mod literal;
#[path = "dial_name_tests.rs"]
mod name;

/// What the scripted connector does for an address.
#[derive(Clone, Copy)]
enum Act {
    Open,
    Refuse,
    Hang,
}

/// A connector for `open_target_with` that records the order of attempts; an address with no entry gets `rest`.
struct Script {
    acts: HashMap<SocketAddr, Act>,
    rest: Act,
    calls: Mutex<Vec<SocketAddr>>,
}

impl Script {
    fn all(rest: Act) -> Script {
        Script {
            acts: HashMap::new(),
            rest,
            calls: Mutex::default(),
        }
    }

    fn with(mut self, addr: &str, act: Act) -> Script {
        self.acts
            .insert(addr.parse().expect("a socket address"), act);
        self
    }

    fn connect(&self, addr: SocketAddr) -> impl Future<Output = io::Result<SocketAddr>> + '_ {
        self.calls.lock().expect("calls").push(addr);
        let act = self.acts.get(&addr).copied().unwrap_or(self.rest);
        async move {
            match act {
                Act::Open => Ok(addr),
                Act::Refuse => Err(io::ErrorKind::ConnectionRefused.into()),
                Act::Hang => std::future::pending().await,
            }
        }
    }

    fn calls(&self) -> Vec<SocketAddr> {
        self.calls.lock().expect("calls").clone()
    }
}

/// The address the exit would connect to: every connection opens at once, the whole budget.
async fn dial(cfg: &ExitConfig, host: &str, port: u16) -> std::result::Result<SocketAddr, Refusal> {
    let script = Script::all(Act::Open);
    open_target_with(cfg, host, port, BUDGET, |addr| script.connect(addr)).await
}

mod open_target {
    use super::*;

    #[tokio::test]
    async fn closed_port_is_connect_failed() {
        let closed = SocketAddr::from(([127, 0, 0, 1], 1));
        let cfg = exit_config(HashMap::from([("closed.test".to_string(), closed)]));

        let refusal = refusal_of(open_target(&cfg, "closed.test", 80).await, "127.0.0.1:1");

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

mod via_interface {
    use super::*;
    use crate::exit::frame::EXIT_CODE_INTERFACE_DOWN;
    use crate::iface_bind::test_support::MISSING_IFACE;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn open_target_binds_socket() {
        // given: a site on 127.0.0.1 and the exit bound to the loopback interface
        let Some(name) = bindable_loopback() else {
            return;
        };
        let (_site, addr) = local_site().await;
        let cfg = exit_config(HashMap::from([("bound.test".to_string(), addr)]));
        cfg.iface.set(Some(&name));

        // when: the target is opened
        let tcp = open_target(&cfg, "bound.test", 80)
            .await
            .expect("the connection opens");

        // then: the socket itself carries the binding, not only the pre-check
        then_bound_to(&tcp, &name);
    }

    #[tokio::test]
    async fn missing_interface_refusal_line() {
        // given: the exit is bound to an interface that doesn't exist
        let (_site, addr) = local_site().await;
        let cfg = exit_config(HashMap::from([("bound.test".to_string(), addr)]));
        cfg.iface.set(Some(MISSING_IFACE));

        // when: the target is opened
        let refusal = refusal_of(open_target(&cfg, "bound.test", 80).await, "bound.test");

        // then: code 8 and a log line naming the target, the interface and the guarantee
        assert_eq!(refusal.code, EXIT_CODE_INTERFACE_DOWN);
        for part in ["bound.test", MISSING_IFACE, "not sent through other networks"] {
            assert!(
                refusal.line.contains(part),
                "the log line has {part:?}: {}",
                refusal.line
            );
        }
    }

    #[tokio::test]
    async fn missing_interface_never_dials_the_target() {
        // given: the exit is bound to a missing interface and the site listens
        let (site, addr) = local_site().await;
        let cfg = exit_config(HashMap::from([("bound.test".to_string(), addr)]));
        cfg.iface.set(Some(MISSING_IFACE));

        // when: the target is opened
        let _ = open_target(&cfg, "bound.test", 80).await;

        // then: no connection reached the site
        let waited = tokio::time::timeout(Duration::from_millis(200), site.accept()).await;
        assert!(waited.is_err(), "the site must see no connection");
    }
}

/// A listening local "site" on `127.0.0.1`; the listener must live as long as the test.
async fn local_site() -> (tokio::net::TcpListener, SocketAddr) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("site listener");
    let addr = listener.local_addr().expect("site address");
    (listener, addr)
}

/// The loopback interface name, or `None` (the test is skipped) where the kernel refuses binding to a device.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn bindable_loopback() -> Option<String> {
    use crate::iface_bind::test_support::loopback_name;
    use crate::iface_bind::{check, BindError};
    let name = loopback_name();
    match check(&name) {
        Ok(()) => Some(name),
        Err(BindError::NoPermission(_)) => {
            eprintln!("skipped: binding to a device needs CAP_NET_RAW on this kernel");
            None
        }
        Err(e) => panic!("the loopback interface {name} must be usable: {e}"),
    }
}

#[cfg(target_os = "linux")]
fn then_bound_to(tcp: &TcpStream, name: &str) {
    let device = socket2::SockRef::from(tcp).device().expect("device option");
    assert_eq!(device, Some(name.as_bytes().to_vec()), "bound device");
}

#[cfg(target_os = "macos")]
fn then_bound_to(tcp: &TcpStream, name: &str) {
    let c_name = std::ffi::CString::new(name).expect("interface name");
    // SAFETY: `c_name` is a valid NUL-terminated string that outlives the call.
    let index = unsafe { libc::if_nametoindex(c_name.as_ptr()) };
    let bound = socket2::SockRef::from(tcp)
        .device_index_v4()
        .expect("bound interface option");
    assert_eq!(bound.map(|i| i.get()), Some(index), "bound interface index");
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
