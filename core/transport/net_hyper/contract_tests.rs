//! Contract suites of the `core/net/contract/` transport on `hyper`: a loopback cluster from `test_cluster`,
//! real DHT connections and UDX. The harness starts the cluster lazily (`$harness()` is called without `await`),
//! `restart` closes the previous node and starts a new one with the same key.

use super::test_cluster::{cluster, Cluster};
use super::{HyperOpts, HyperTransport};
use crate::transport::net::contract::transport::transport_contract;
use crate::transport::net::contract::transport_diag::transport_diag_contract;
use crate::transport::net::contract::{secret, Harness};
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::{Transport, AUTH, PORT};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::sync::OnceCell;

transport_contract!(hyper_harness);
transport_diag_contract!(hyper_harness);

/// Nodes by seeds and the cluster they live on.
struct HyperHarness {
    cluster: OnceCell<Cluster>,
    nodes: Mutex<HashMap<u8, Arc<HyperTransport>>>,
}

impl HyperHarness {
    async fn bootstrap(&self) -> Vec<String> {
        self.cluster.get_or_init(|| cluster(3)).await.bootstrap()
    }

    /// New node: `serve([AUTH, PORT])`, then waiting for the `listen` outcome.
    async fn start(&self, seed: u8) -> Arc<dyn Transport> {
        let opts = HyperOpts {
            bootstrap: Some(self.bootstrap().await),
            ..Default::default()
        };
        let node = HyperTransport::bind(secret(seed), opts)
            .await
            .expect("node came up");
        node.serve(&[AUTH, PORT]);
        node.wait_listen_outcome().await.expect("listen succeeded");
        self.remember(seed, node.clone());
        node
    }

    fn remember(&self, seed: u8, node: Arc<HyperTransport>) {
        self.nodes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(seed, node);
    }

    fn forget(&self, seed: u8) -> Option<Arc<HyperTransport>> {
        self.nodes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&seed)
    }
}

impl Harness for HyperHarness {
    fn node(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Transport>> {
        Box::pin(self.start(seed))
    }

    fn restart(&self, seed: u8) -> BoxFuture<'_, Arc<dyn Transport>> {
        Box::pin(async move {
            if let Some(old) = self.forget(seed) {
                old.close().await;
            }
            self.start(seed).await
        })
    }

    // Workaround for DIAL_SLOTS: a node holds no more than 8 outgoing
    // connections, a hundred streams go slower; restore 5 s (STEP) once the limit is lifted.
    fn hundred_streams_limit(&self) -> Duration {
        Duration::from_secs(15)
    }
}

fn hyper_harness() -> HyperHarness {
    HyperHarness {
        cluster: OnceCell::new(),
        nodes: Mutex::new(HashMap::new()),
    }
}
