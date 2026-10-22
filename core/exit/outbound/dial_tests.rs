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
