//! Contract suites for any connection, transport and discovery; compiled only in test builds.
//! An adapter plugs a suite in with one macro line in its own `contract_tests.rs`; this file holds the shared helpers.

pub(crate) mod connector;
pub(crate) mod discovery;
pub(crate) mod membership;
pub(crate) mod replica;
pub(crate) mod replica_cases;
pub(crate) mod replica_sync_cases;
pub(crate) mod transport;
pub(crate) mod transport_cases;
pub(crate) mod transport_diag;

use crate::net::connect::Connector;
use crate::net::discovery::Discovery;
use crate::net::id::{NodeSecret, PeerId};
use crate::net::replica::ReplicaEntry;
use crate::net::stream::BoxFuture;
use crate::net::sync_backend::SyncBackend;
use crate::net::transport::Transport;
use std::any::Any;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// Time limit for one suite step: past it the test has failed, not hung.
pub(crate) const STEP: Duration = Duration::from_secs(5);

/// Node seeds used by the suites: `secret(SEED_A)` is the key of node A, and so on.
pub(crate) const SEED_A: u8 = 1;
pub(crate) const SEED_B: u8 = 2;
pub(crate) const SEED_C: u8 = 3;

/// Environment of the transport suite: nodes of one environment, each with `serve([AUTH, PORT])` already called.
pub(crate) trait Harness {
    /// A new node with the key `NodeSecret::from_bytes(&[seed; 32])`.
    fn node(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Transport>>;
    /// Restarts a node with the same key: the old one is closed, the address is new, `serve([AUTH, PORT])` is called.
    fn restart(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Transport>>;
    /// Time limit for the "hundred streams at once" case; `STEP` by default.
    fn hundred_streams_limit(&self) -> Duration {
        STEP
    }
}

/// Environment of the connection suite: the same nodes, but without a data stream and services.
pub(crate) trait ConnectorHarness {
    /// A new connection with the key `NodeSecret::from_bytes(&[seed; 32])`.
    fn node(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Connector>>;
    /// Restarts a node with the same key: the old connection is closed, the address is new.
    fn restart(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Connector>>;
}

/// Environment of the discovery suite: the sources share one environment (board, DHT, directory).
pub(crate) trait DiscoveryHarness {
    /// A new source; `publishes` tells whether it publishes its own address.
    fn source(&self, publishes: bool) -> Arc<dyn Discovery>;
}

/// "A foreign entry as if it arrived via reconciliation": puts a `ReplicaEntry` with the given author, key, value and
/// `ts_micros` into the `"net"` replica of node `a`; subscribers see `RemoteChange`. Only implementations that can do
/// this (`mem`) have it; for the others the implementation sets the time and author itself.
pub(crate) type RawPut = Arc<dyn Fn(ReplicaEntry) + Send + Sync>;

/// Environment of the sync suites (replica and membership): two nodes of one network.
///
/// Harness conventions: `async fn() -> SyncPair`, a fresh environment for every test:
/// - node `a` has the key `secret(SEED_A)`, node `b` has `secret(SEED_B)`; both are in one network (one `network_seed`);
/// - `a` already knows how to reach `b` and vice versa (addresses have been exchanged);
/// - the suite opens replicas and topics itself (`open_replica`, `join_swarm`); reconciliation is switched on only by it (`start_sync`);
/// - `_keep` holds everything that must live until the end of the test (transports, network) and is dropped together with `SyncPair`;
/// - `raw` is `Some(f)` if the implementation can accept a foreign entry as if it arrived via reconciliation, otherwise `None`.
pub(crate) struct SyncPair {
    pub a: Arc<dyn SyncBackend>,
    pub b: Arc<dyn SyncBackend>,
    pub _keep: Box<dyn Any + Send>,
    pub raw: Option<RawPut>,
}

/// Node key from a seed.
pub(crate) fn secret(seed: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[seed; 32])
}

/// Node id from a seed.
pub(crate) fn pid(seed: u8) -> PeerId {
    secret(seed).peer_id()
}

pub(crate) async fn must_finish<T>(fut: impl Future<Output = T>) -> T {
    must_finish_within(STEP, fut).await
}

/// A suite step with its own time limit (heavy transport cases).
pub(crate) async fn must_finish_within<T>(limit: Duration, fut: impl Future<Output = T>) -> T {
    match tokio::time::timeout(limit, fut).await {
        Ok(value) => value,
        Err(_) => panic!("suite step did not finish within {limit:?}"),
    }
}
