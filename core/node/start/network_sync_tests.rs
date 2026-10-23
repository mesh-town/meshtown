//! The node's sync is chosen from the registry by name: the same consumer checks
//! (exit flag, node values, ports, domain list, invites) run on the `mem` sync and on `mesh`
//! (`consumer_suite!(mem)`, `consumer_suite!(mesh)`). We wait for a peer's data by polling up to `STEP`;
//! the `Synced` event does not do as "the data arrived".

use crate::invite::store::{issue_record, IssueVerdict, Ledger};
use crate::node::node_state::{announce_exit, own_value, read_exit_flags, read_values};
use crate::ports_sharing::{Audience, PortsDoc, Published};
use crate::proxy::domains::{DomainList, LIST_PREFIX};
use crate::transport::net::contract::{SEED_A, SEED_B, STEP};
use crate::transport::net::mem::{sync_ctor as mem_ctor, MemNet, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{
    NetChoice, NetError, NetResult, NetStack, NetStackBuilder, NodeSecret, Registry, SyncBackend,
    Transport,
};
use bytes::Bytes;
use std::any::Any;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// The polling period for a peer's data.
const POLL: Duration = Duration::from_millis(20);
/// The node value key for checking `own_value` / `read_values`.
const VALUE_KEY: &[u8] = b"node/test/";
/// The label of the shared replica: its arrival at the peer proves that reconciliation took place.
const MARK_KEY: &[u8] = b"test/sync/";
/// The hash of the invite secret (hex digits).
const INVITE_HASH: &str = "cafe0123456789abcdef0123456789abcdef0123456789abcdef0123456789ab";

macro_rules! consumer_suite {
    ($name:ident) => {
        mod $name {
            use super::*;

            #[tokio::test]
            async fn exit_flag_seen_by_neighbor() {
                // given: two sync nodes, A has announced the exit
                let p = setup(stringify!($name)).await;
                announce_exit(&*p.a.shared_replica(), true).await.unwrap();
                // when: A reconciles with B
                p.sync_a_with_b().await;
                // then: B, by polling, sees A as an exit
                let (b, a_id) = (&p.b, p.a.me());
                eventually("A's exit flag at B", move || async move {
                    let exits = read_exit_flags(&*b.shared_replica()).await.unwrap();
                    exits.get(&a_id) == Some(&true)
                })
                .await;
            }

            #[tokio::test]
            async fn node_value_seen_by_neighbor() {
                // given: A wrote a node value under its own key
                let p = setup(stringify!($name)).await;
                p.a.shared_replica()
                    .set_own(VALUE_KEY, Bytes::from_static(b"ru"))
                    .await
                    .unwrap();
                let own = own_value(&*p.a.shared_replica(), VALUE_KEY).await.unwrap();
                assert_eq!(
                    own.map(|v| v.0),
                    Some(Bytes::from_static(b"ru")),
                    "A's own value"
                );
                // when: A reconciles with B
                p.sync_a_with_b().await;
                // then: B, by polling, reads A's value
                let (b, a_id) = (&p.b, p.a.me());
                eventually("A's value at B", move || async move {
                    let values = read_values(&*b.shared_replica(), VALUE_KEY).await.unwrap();
                    values.get(&a_id) == Some(&Bytes::from_static(b"ru"))
                })
                .await;
            }

            #[tokio::test]
            async fn ports_seen_by_neighbor() {
                // given: A published port 8080 named "web"
                let p = setup(stringify!($name)).await;
                PortsDoc::new(p.a.shared_replica())
                    .publish(&published(8080, "web"))
                    .await
                    .unwrap();
                // when: A reconciles with B
                p.sync_a_with_b().await;
                // then: B, by polling, sees A's publication with a parsed record
                let (b, a_id) = (&p.b, p.a.me());
                eventually("A's publication at B", move || async move {
                    let views = PortsDoc::new(b.shared_replica()).read_all().await.unwrap();
                    let from_a = views.iter().find(|v| (v.author, v.port) == (a_id, 8080));
                    let name = from_a.and_then(|v| v.record.clone()).map(|r| r.name);
                    name.as_deref() == Some("web")
                })
                .await;
            }

            #[tokio::test]
            async fn domain_list_stays_private() {
                // given: A added a domain to the list and put a label into the shared replica
                let p = setup(stringify!($name)).await;
                p.a.add("example.com").await.unwrap();
                p.a.shared_replica()
                    .set_own(MARK_KEY, Bytes::from_static(b"v1"))
                    .await
                    .unwrap();
                assert_eq!(p.a.list().await.unwrap(), ["example.com"], "A's list");
                // when: A reconciles with B and the label arrived (reconciliation happened)
                p.sync_a_with_b().await;
                let b = &p.b;
                eventually("A's label at B", move || async move {
                    !b.shared_replica().scan(MARK_KEY).await.unwrap().is_empty()
                })
                .await;
                // then: B's list is empty, and A's domain did not land in the shared replica
                assert!(
                    p.b.list().await.unwrap().is_empty(),
                    "B's list is not empty"
                );
                let leaked =
                    p.b.shared_replica()
                        .scan(LIST_PREFIX.as_bytes())
                        .await
                        .unwrap();
                assert!(
                    leaked.is_empty(),
                    "the domain list leaked into the shared replica: {leaked:?}"
                );
            }

            #[tokio::test]
            async fn invite_ledger_verdict() {
                // given: A issued an invite; B doesn't know about it yet
                let p = setup(stringify!($name)).await;
                let now = crate::transport::net_mainline::unix_now();
                let (ledger_a, ledger_b) = (Ledger::of(&p.a), Ledger::of(&p.b));
                ledger_a
                    .issue(INVITE_HASH, &issue_record(now, None))
                    .await
                    .unwrap();
                assert_eq!(
                    ledger_b.verdict(INVITE_HASH, now).await,
                    IssueVerdict::Unknown
                );
                // when: A reconciles with B
                p.sync_a_with_b().await;
                // then: B, by polling, reaches the verdict "grant"
                let ledger = &ledger_b;
                eventually("Grant verdict at B", move || async move {
                    ledger.verdict(INVITE_HASH, now).await == IssueVerdict::Grant
                })
                .await;
            }
        }
    };
}

consumer_suite!(mem);
consumer_suite!(mesh);

/// The node registry (`sync_registry`) opens every collected sync name, and the opened sync has the same name.
#[tokio::test]
async fn node_registry_opens_every_compiled_sync() {
    // given: the node registry on a loopback transport
    let registry = super::sync_registry();
    for name in crate::node::net_names::sync() {
        // when: the sync is opened by a name from the collected list
        let sync = open_sync(&registry, &choice_with_sync(name), SEED_A).await;
        // then: the name of the opened sync equals the requested one
        assert_eq!(
            sync.name(),
            name,
            "the node registry opened a different sync"
        );
    }
}

/// The node registry rejects an uncollected sync name with a settings error listing the available ones.
#[tokio::test]
async fn node_registry_rejects_unknown_sync() {
    // given: the node registry and a choice with a nonexistent sync "x"
    let registry = super::sync_registry();
    // when: trying to open the sync "x"
    let result = try_open_sync(&registry, &choice_with_sync("x"), SEED_A).await;
    // then: a settings error, the text lists the available names
    let Err(err) = result else {
        panic!("sync \"x\" must not open");
    };
    assert!(
        matches!(err, NetError::Config(_)),
        "expected NetError::Config, got {err:?}"
    );
    assert!(
        err.to_string().contains("mem") && err.to_string().contains("mesh"),
        "the text has no list of available syncs: {err}"
    );
}

/// Two nodes of one network with the `sync` sync chosen from the registry by name.
struct Pair {
    a: DomainList,
    b: DomainList,
    /// Holds the syncs, transports and the `mem` network until the end of the test.
    _keep: Box<dyn Any + Send>,
}

impl Pair {
    async fn sync_a_with_b(&self) {
        self.a
            .sync_shared_with(self.b.me())
            .await
            .expect("A's reconciliation with B starts");
    }
}

/// Nodes A and B: a registry with `"mem"`, `open_sync` by name, then domain lists on the sync replicas.
async fn setup(sync: &str) -> Pair {
    let net = MemSyncNet::new();
    let choice = NetChoice {
        sync: sync.to_string(),
        ..NetChoice::default()
    };
    let (sa, sb) = (
        open_sync(&registry(&net), &choice, SEED_A).await,
        open_sync(&registry(&net), &choice, SEED_B).await,
    );
    assert_eq!(
        (sa.name(), sb.name()),
        (sync, sync),
        "the registry chose a different sync"
    );
    let stacks = (sync == "mesh").then(|| join_mesh([&sa, &sb]));
    let (a, b) = (open_list(&*sa).await, open_list(&*sb).await);
    Pair {
        a,
        b,
        _keep: Box::new((net, sa, sb, stacks)),
    }
}

/// `mesh` lives on top of the stack: both nodes are in one `mem` network, A knows B's address (and vice versa), the syncs
/// get stacks before the first replica.
fn join_mesh(syncs: [&Arc<dyn SyncBackend>; 2]) -> Vec<NetStack> {
    let net = MemNet::new();
    let seeds = [SEED_A, SEED_B];
    let stacks: Vec<_> = syncs
        .iter()
        .zip(seeds)
        .map(|(sync, seed)| {
            let secret = NodeSecret::from_bytes(&[seed; 32]);
            let transport: Arc<dyn Transport> = net.transport(&secret);
            let mut builder = NetStackBuilder::new(secret).transport(transport);
            for (id, handler) in sync.services() {
                builder = builder.serve(id, handler);
            }
            let stack = builder.build().expect("stack built");
            sync.attach_stack(stack.downgrade());
            stack
        })
        .collect();
    for (stack, other) in [(&stacks[0], SEED_B), (&stacks[1], SEED_A)] {
        let id = NodeSecret::from_bytes(&[other; 32]).peer_id();
        stack.add_hints(net.addr_of(id).expect("peer slot is alive"));
    }
    stacks
}

/// The sync registry: in memory and `mesh` (its own replicas on top of `mem`).
fn registry(net: &Arc<MemSyncNet>) -> Registry {
    let mut registry = Registry::new();
    registry.add_sync("mem", mem_ctor(net.clone()));
    registry.add_sync(
        crate::members::sync::replica::NAME,
        crate::members::sync::replica::sync_ctor(),
    );
    registry
}

async fn open_sync(registry: &Registry, choice: &NetChoice, seed: u8) -> Arc<dyn SyncBackend> {
    let ctx = SyncCtx {
        secret: NodeSecret::from_bytes(&[seed; 32]),
        network_seed: [9; 32],
        store_dir: None,
    };
    registry
        .open_sync(choice, ctx)
        .await
        .expect("sync opens by name")
}

async fn open_list(sync: &dyn SyncBackend) -> DomainList {
    let shared = sync
        .open_replica(REPLICA_NET)
        .await
        .expect("shared replica");
    let own = sync
        .open_replica(REPLICA_DOMAINS)
        .await
        .expect("own list replica");
    DomainList::open(shared, own)
        .await
        .expect("domain list opens")
}

fn published(port: u16, name: &str) -> Published {
    Published {
        port,
        name: name.to_string(),
        audience: Audience::All,
    }
}

/// Polls `check` until it returns `true`; if it doesn't make it within `STEP` — panics with the name of the wait.
async fn eventually<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let polling = async {
        while !check().await {
            tokio::time::sleep(POLL).await;
        }
    };
    if tokio::time::timeout(STEP, polling).await.is_err() {
        panic!("timed out after {STEP:?}: {what}");
    }
}

/// A network choice with defaults and the `sync` sync.
fn choice_with_sync(sync: &str) -> NetChoice {
    NetChoice {
        sync: sync.to_string(),
        ..NetChoice::default()
    }
}

/// Like `open_sync`, but returns the result: for checking refusals.
async fn try_open_sync(
    registry: &Registry,
    choice: &NetChoice,
    seed: u8,
) -> NetResult<Arc<dyn SyncBackend>> {
    let ctx = SyncCtx {
        secret: NodeSecret::from_bytes(&[seed; 32]),
        network_seed: [9; 32],
        store_dir: None,
    };
    registry.open_sync(choice, ctx).await
}
