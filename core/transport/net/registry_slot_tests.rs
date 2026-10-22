//! Tests for the sync slot in the registry (`registry.rs`): `add_sync` and `open_sync` — choice by name from `NetChoice`.
//! In a separate file: `registry_tests.rs` is already at the line ceiling.

use crate::net::contract::secret;
use crate::net::error::{NetError, NetResult};
use crate::net::id::PeerId;
use crate::net::registry::{NetChoice, Registry};
use crate::net::replica::Replica;
use crate::net::stream::BoxFuture;
use crate::net::sync_backend::{Swarm, SyncBackend, SyncCtor, SyncCtx};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn open_sync_unknown_name_is_config_error() {
    // given: a registry with a single sync "mem"
    let calls = counter();
    let mut reg = Registry::new();
    reg.add_sync("mem", stub_ctor("mem", &calls));
    // when: we open the sync named "x"
    let result = reg.open_sync(&choice("x"), ctx()).await;
    // then: Config, the text names "x" and lists what is registered
    let msg = config_msg(result);
    assert_eq!(msg, "sync \"x\" is not registered; available: mem");
    assert_eq!(count(&calls), 0, "the \"mem\" constructor must not be called");
}

#[tokio::test]
async fn open_sync_picks_by_name() {
    // given: a registry with syncs "a" and "b", each with its own name()
    let (calls_a, calls_b) = (counter(), counter());
    let mut reg = Registry::new();
    reg.add_sync("a", stub_ctor("a", &calls_a));
    reg.add_sync("b", stub_ctor("b", &calls_b));
    // when: we open the sync "b"
    let backend = reg
        .open_sync(&choice("b"), ctx())
        .await
        .unwrap_or_else(|e| panic!("open_sync(\"b\") did not open the sync: {e}"));
    // then: we got exactly "b", the constructor of "a" was not called
    assert_eq!(backend.name(), "b");
    assert_eq!(count(&calls_b), 1, "the \"b\" constructor is called once");
    assert_eq!(count(&calls_a), 0, "the \"a\" constructor must not be called");
}

/// Minimal sync: remembers only its name, provides no network or replicas.
struct StubSync {
    name: &'static str,
}

impl SyncBackend for StubSync {
    fn name(&self) -> &'static str {
        self.name
    }

    fn join_swarm(
        &self,
        _topic: [u8; 32],
        _bootstrap: &[PeerId],
    ) -> BoxFuture<'_, NetResult<Swarm>> {
        Box::pin(async {
            Err(NetError::Config("the stub does not join topics".into()))
        })
    }

    fn open_replica(&self, _name: &str) -> BoxFuture<'_, NetResult<Arc<dyn Replica>>> {
        Box::pin(async {
            Err(NetError::Config("the stub does not open replicas".into()))
        })
    }
}

/// Stub constructor: each call adds one to the counter.
fn stub_ctor(name: &'static str, calls: &Arc<AtomicUsize>) -> SyncCtor {
    let calls = calls.clone();
    Box::new(move |_ctx| {
        calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(Arc::new(StubSync { name }) as Arc<dyn SyncBackend>) })
    })
}

fn choice(sync: &str) -> NetChoice {
    NetChoice {
        sync: sync.into(),
        ..NetChoice::default()
    }
}

fn ctx() -> SyncCtx {
    SyncCtx {
        secret: secret(1),
        network_seed: [1; 32],
        store_dir: None,
    }
}

fn counter() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

fn count(calls: &Arc<AtomicUsize>) -> usize {
    calls.load(Ordering::SeqCst)
}

/// The `Config` text; any other outcome is a panic with a description.
fn config_msg(result: NetResult<Arc<dyn SyncBackend>>) -> String {
    match result {
        Err(NetError::Config(msg)) => msg,
        Err(other) => panic!("expected Config, got another error: {other}"),
        Ok(backend) => panic!("expected Config, got the sync \"{}\"", backend.name()),
    }
}
