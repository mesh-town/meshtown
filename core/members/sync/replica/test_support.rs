//! The test scaffold of the own replica on `mem`: a node with a stack and `MeshSync`, address exchange, a temporary directory.

use super::host::ReplicaHost;
use super::MeshSync;
use crate::members::sync::flood::test_support::{fast, NETWORK};
use crate::transport::net::contract::{secret, SyncPair, SEED_A, SEED_B};
use crate::transport::net::mem::MemNet;
use crate::transport::net::replica::ReplicaEntry;
use crate::transport::net::sync_backend::{SyncBackend, SyncCtx};
use crate::transport::net::{NetStack, NetStackBuilder};
use portable_atomic::AtomicU64;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub(crate) struct MeshNode {
    pub sync: Arc<MeshSync>,
    pub stack: NetStack,
}

/// Node `seed` on network `net`: a sync over directory `dir` (`None` — memory), the stack serves `services()`.
pub(crate) async fn mesh_node(
    net: &MemNet,
    seed: u8,
    network_seed: [u8; 32],
    dir: Option<PathBuf>,
) -> MeshNode {
    let ctx = SyncCtx {
        secret: secret(seed),
        network_seed,
        store_dir: dir,
    };
    let host = ReplicaHost::load(&ctx, None)
        .await
        .expect("the replicas came up");
    let sync = MeshSync::assemble(&ctx, host, fast());
    let mut builder = NetStackBuilder::new(secret(seed)).transport(net.transport(&secret(seed)));
    for (id, handler) in sync.services() {
        builder = builder.serve(id, handler);
    }
    let stack = builder.build().expect("the stack was built");
    sync.attach_stack(stack.downgrade());
    MeshNode { sync, stack }
}

/// The nodes know each other's addresses (without hints `mem` answers `NoRoute`).
pub(crate) fn introduce(a: &MeshNode, b: &MeshNode) {
    a.stack.add_hints(b.stack.invite_addrs()[0].clone());
    b.stack.add_hints(a.stack.invite_addrs()[0].clone());
}

/// The contract harness: two in-memory nodes of one network; `raw` puts an entry into node `a`'s `"net"`.
pub(crate) async fn mesh_mem_pair() -> SyncPair {
    let net = MemNet::new();
    let a = mesh_node(&net, SEED_A, NETWORK, None).await;
    let b = mesh_node(&net, SEED_B, NETWORK, None).await;
    introduce(&a, &b);
    let target = a.sync.host.open("net").expect("the net replica exists");
    let raw: Arc<dyn Fn(ReplicaEntry) + Send + Sync> = Arc::new(move |e| target.put_raw(e));
    SyncPair {
        a: a.sync.clone(),
        b: b.sync.clone(),
        _keep: Box::new((net, a, b)),
        raw: Some(raw),
    }
}

/// A fresh empty test directory.
pub(crate) fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-replica-{tag}-{}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("test directory");
    dir
}
