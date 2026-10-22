//! Tests for the registry (`registry.rs`): default choice, name checks, constructor order, discovery priority.

use crate::transport::net::discovery::Discovery;
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::facade::NetStackBuilder;
use crate::transport::net::id::PeerId;
use crate::transport::net::mem::MemNet;
use crate::transport::net::registry::{DiscoveryCtor, NetChoice, NetCtx, Registry, TransportCtor};
use crate::transport::net::transport::Transport;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

mod default_choice {
    use super::*;

    #[test]
    fn default_is_today() {
        let today = NetChoice::default();

        assert_eq!(today.transports, vec!["hyper"]);
        assert_eq!(today.discovery, vec!["invite", "cache", "hyperdht"]);
        assert_eq!(today.sync, "mesh");
    }

    #[test]
    fn clones_and_compares_equal() {
        let today = NetChoice::default();

        assert_eq!(today.clone(), today);
        assert_ne!(
            NetChoice {
                sync: "other".into(),
                ..today.clone()
            },
            today
        );
    }
}

mod unknown_names {
    use super::*;

    #[test]
    fn unknown_transport_lists_sorted() {
        let mut reg = Registry::new();
        reg.add_transport("zeta", failing_transport("zeta"));
        reg.add_transport("alpha", failing_transport("alpha"));

        let msg = config_msg(reg.builder(&choice(&["iroh"], &[]), &ctx(1)));

        assert_eq!(
            msg,
            "transport \"iroh\" is not registered; available: alpha, zeta"
        );
    }

    #[test]
    fn unknown_discovery_lists() {
        let mut reg = Registry::new();
        reg.add_discovery("b", failing_discovery("b"));
        reg.add_discovery("a", failing_discovery("a"));

        let msg = config_msg(reg.builder(&choice(&[], &["nope"]), &ctx(1)));

        assert_eq!(
            msg,
            "discovery source \"nope\" is not registered; available: a, b"
        );
    }

    #[test]
    fn empty_registry_says_none() {
        let reg = Registry::new();

        let msg = config_msg(reg.builder(&choice(&["iroh"], &[]), &ctx(1)));

        assert_eq!(msg, "transport \"iroh\" is not registered; available: none");
    }

    #[test]
    fn empty_registry_says_none_for_discovery() {
        let reg = Registry::new();

        let msg = config_msg(reg.builder(&choice(&[], &["invite"]), &ctx(1)));

        assert_eq!(
            msg,
            "discovery source \"invite\" is not registered; available: none"
        );
    }

    #[tokio::test]
    async fn names_are_checked_before_any_constructor() {
        let calls = counter();
        let mut reg = Registry::new();
        reg.add_transport("mem", mem_transport(&calls, &Log::default()));

        let res = reg.builder(&choice(&["mem"], &["nope"]), &ctx(1));

        config_msg(res);
        assert_eq!(
            count(&calls),
            0,
            "the transport constructor must not be called"
        );
    }
}

mod duplicates_in_choice {
    use super::*;

    #[tokio::test]
    async fn duplicate_in_choice_no_ctor_called() {
        let calls = counter();
        let mut reg = Registry::new();
        reg.add_transport("mem", mem_transport(&calls, &Log::default()));

        let msg = config_msg(reg.builder(&choice(&["mem", "mem"], &[]), &ctx(1)));

        assert!(
            msg.contains("\"mem\"") && msg.contains("is specified twice"),
            "text: {msg}"
        );
        assert_eq!(
            count(&calls),
            0,
            "the constructors have not been called yet"
        );
    }

    #[test]
    fn duplicate_discovery_in_choice_is_config() {
        let mut reg = Registry::new();
        reg.add_discovery("a", failing_discovery("a"));

        let msg = config_msg(reg.builder(&choice(&[], &["a", "a"]), &ctx(1)));

        assert!(
            msg.contains("\"a\"") && msg.contains("is specified twice"),
            "text: {msg}"
        );
    }
}

mod constructors {
    use super::*;

    #[tokio::test]
    async fn ctor_error_propagates() {
        let log = Log::default();
        let mut reg = Registry::new();
        reg.add_transport("a", mem_transport(&counter(), &log));
        reg.add_transport("b", failing_transport("boom"));

        let msg = config_msg(reg.builder(&choice(&["a", "b"], &[]), &ctx(1)));

        assert_eq!(msg, "boom", "the constructor's error goes as is");
        assert!(
            log.alive().is_empty(),
            "the transport created earlier must be dropped"
        );
        assert_eq!(log.made(), 1, "control: the first constructor ran");
    }

    #[tokio::test]
    async fn discovery_ctor_error_propagates() {
        let log = Log::default();
        let mut reg = Registry::new();
        reg.add_transport("mem", mem_transport(&counter(), &log));
        reg.add_discovery("bad", failing_discovery("kaput"));

        let msg = config_msg(reg.builder(&choice(&["mem"], &["bad"]), &ctx(1)));

        assert_eq!(msg, "kaput");
        assert!(
            log.alive().is_empty(),
            "the transport is dropped together with the builder"
        );
    }

    #[tokio::test]
    async fn ctor_gets_ctx_secret() {
        let log = Log::default();
        let mut reg = Registry::new();
        reg.add_transport("mem", mem_transport(&counter(), &log));

        let builder = reg
            .builder(&choice(&["mem"], &[]), &ctx(7))
            .expect("the builder succeeded");

        assert_eq!(
            log.secrets(),
            vec![pid(7)],
            "the constructor received the secret from ctx"
        );
        assert!(
            builder.build().is_ok(),
            "a transport with the ctx key passes the key check"
        );
    }

    #[tokio::test]
    async fn ctors_are_called_in_choice_order() {
        let log = Log::default();
        let mut reg = Registry::new();
        reg.add_transport("a", mem_transport(&counter(), &log.named("a")));
        reg.add_transport("b", mem_transport(&counter(), &log.named("b")));

        reg.builder(&choice(&["b", "a"], &[]), &ctx(1))
            .expect("the builder succeeded");

        assert_eq!(
            log.order(),
            vec!["b", "a"],
            "order is as in the choice, not alphabetical"
        );
    }

    #[tokio::test]
    async fn sync_ignored() {
        let mut reg = Registry::new();
        reg.add_transport("mem", mem_transport(&counter(), &Log::default()));
        let wanted = NetChoice {
            sync: "nowhere".into(),
            ..choice(&["mem"], &[])
        };

        let res = reg.builder(&wanted, &ctx(1));

        assert!(res.is_ok(), "sync is not checked at this stage");
    }

    #[tokio::test]
    async fn empty_discovery_ok_but_empty_transports_fail_in_build() {
        let mut reg = Registry::new();
        reg.add_transport("mem", mem_transport(&counter(), &Log::default()));

        let with_none = reg.builder(&choice(&["mem"], &[]), &ctx(1));
        let without = reg.builder(&choice(&[], &[]), &ctx(1));

        assert!(with_none.is_ok(), "empty discovery is allowed");
        let built = without
            .expect("an empty transport list is caught not by the builder")
            .build();
        assert!(matches!(built, Err(NetError::Config(_))), "but by build");
    }
}

mod priority {
    use super::*;

    #[tokio::test]
    async fn priority_is_choice_order() {
        let net = MemNet::new();
        let mut reg = Registry::new();
        reg.add_transport("mem", mem_transport(&counter(), &Log::default()));
        reg.add_discovery("aa", board(&net, "aa"));
        reg.add_discovery("zz", board(&net, "zz"));

        let stack = built(&reg, &choice(&["mem"], &["zz", "aa"]));
        let reversed = built(&reg, &choice(&["mem"], &["aa", "zz"]));

        assert_eq!(
            stack.snapshot().discovery_names,
            vec!["zz", "aa"],
            "choice order, not alphabetical"
        );
        assert_eq!(reversed.snapshot().discovery_names, vec!["aa", "zz"]);
    }
}

mod adding {
    use super::*;

    #[test]
    #[should_panic(expected = "is already added")]
    fn add_twice_panics() {
        let mut reg = Registry::new();
        reg.add_transport("mem", failing_transport("one"));

        reg.add_transport("mem", failing_transport("two"));
    }

    #[test]
    #[should_panic(expected = "is already added")]
    fn add_discovery_twice_panics() {
        let mut reg = Registry::new();
        reg.add_discovery("mainline", failing_discovery("one"));

        reg.add_discovery("mainline", failing_discovery("two"));
    }

    #[test]
    fn same_name_in_transports_and_discovery_is_fine() {
        let mut reg = Registry::new();

        reg.add_transport("mem", failing_transport("t"));
        reg.add_discovery("mem", failing_discovery("d"));
    }
}

/// What the constructors saw: whose secrets, in what order, which transports are still alive.
#[derive(Clone, Default)]
struct Log {
    name: Option<&'static str>,
    secrets: Arc<Mutex<Vec<PeerId>>>,
    order: Arc<Mutex<Vec<&'static str>>>,
    made: Arc<Mutex<Vec<std::sync::Weak<dyn Transport>>>>,
}

impl Log {
    /// The same log, but the constructor writes this name into the order.
    fn named(&self, name: &'static str) -> Log {
        Log {
            name: Some(name),
            ..self.clone()
        }
    }

    fn secrets(&self) -> Vec<PeerId> {
        self.secrets.lock().unwrap().clone()
    }

    fn order(&self) -> Vec<&'static str> {
        self.order.lock().unwrap().clone()
    }

    fn made(&self) -> usize {
        self.made.lock().unwrap().len()
    }

    /// Created transports that have not been destroyed yet.
    fn alive(&self) -> Vec<Arc<dyn Transport>> {
        self.made
            .lock()
            .unwrap()
            .iter()
            .filter_map(|w| w.upgrade())
            .collect()
    }
}

fn counter() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

fn count(calls: &Arc<AtomicUsize>) -> usize {
    calls.load(Ordering::SeqCst)
}

fn ctx(seed: u8) -> NetCtx {
    NetCtx {
        secret: secret(seed),
    }
}

fn choice(transports: &[&str], discovery: &[&str]) -> NetChoice {
    let own = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
    NetChoice {
        transports: own(transports),
        discovery: own(discovery),
        sync: "iroh-docs".into(),
    }
}

/// Constructor of the `mem` transport on its own network: counts calls, writes the secret and the order into the log.
fn mem_transport(calls: &Arc<AtomicUsize>, log: &Log) -> TransportCtor {
    let (calls, log, net) = (calls.clone(), log.clone(), MemNet::new());
    Box::new(move |ctx: &NetCtx| {
        calls.fetch_add(1, Ordering::SeqCst);
        log.secrets.lock().unwrap().push(ctx.secret.peer_id());
        log.order.lock().unwrap().extend(log.name);
        let made: Arc<dyn Transport> = net.transport(&ctx.secret);
        log.made.lock().unwrap().push(Arc::downgrade(&made));
        Ok(made)
    })
}

fn failing_transport(msg: &'static str) -> TransportCtor {
    Box::new(move |_| Err(NetError::Config(msg.to_string())))
}

fn failing_discovery(msg: &'static str) -> DiscoveryCtor {
    Box::new(move |_| Err(NetError::Config(msg.to_string())))
}

/// Constructor of the `mem` source with the board `name` of the shared network.
fn board(net: &Arc<MemNet>, name: &'static str) -> DiscoveryCtor {
    let net = net.clone();
    Box::new(move |_| {
        let source: Arc<dyn Discovery> = net.discovery(name, false);
        Ok(source)
    })
}

/// The `Config` message from the builder's error; another error or a success fails the test.
fn config_msg(res: NetResult<NetStackBuilder>) -> String {
    match res {
        Err(NetError::Config(msg)) => msg,
        Err(other) => panic!("expected Config, got {other:?}"),
        Ok(_) => panic!("expected Config, got a builder"),
    }
}

fn built(reg: &Registry, wanted: &NetChoice) -> crate::transport::net::facade::NetStack {
    let builder = reg.builder(wanted, &ctx(1)).expect("the builder succeeded");
    builder
        .build()
        .unwrap_or_else(|e| panic!("the build would succeed: {e}"))
}
