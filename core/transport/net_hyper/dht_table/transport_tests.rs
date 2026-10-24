//! The saved table through `HyperTransport`: the file is read before the node starts and written on close.

use super::{parse, render, TTL};
use crate::transport::net::contract::secret;
use crate::transport::net::service::{AUTH, PORT};
use crate::transport::net::Transport;
use crate::transport::net_hyper::test_cluster::{cluster, Cluster};
use crate::transport::net_hyper::{HyperOpts, HyperTransport};
use crate::transport::net_mainline::unix_now;
use std::net::SocketAddrV4;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

const WITHIN: Duration = Duration::from_secs(15);
/// Nothing listens here: used as a start node that never answers.
const CLOSED_START: &str = "127.0.0.1:9";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn saved_table_joins_without_bootstrap() {
    let w = setup("joins").await;
    let c = w.serving_node(3, w.opts(w.cluster.bootstrap(), None)).await;
    w.saved_table_of_node(1).await;

    let b = w
        .node(2, w.opts(closed_start(), Some(w.file.clone())))
        .await;
    timeout(WITHIN, b.wait_listen_outcome())
        .await
        .expect("B's announcement hung")
        .expect("B's announcement");
    let res = timeout(WITHIN, b.connect(&c.local_addr(), PORT)).await;

    assert!(
        matches!(res, Ok(Ok(_))),
        "B must reach C through the saved table alone: {:?}",
        res.as_ref().map(|r| r.as_ref().map(|_| ()))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_table_and_no_bootstrap_cannot_dial() {
    let w = setup("control").await;
    let c = w.serving_node(3, w.opts(w.cluster.bootstrap(), None)).await;

    let b = w.node(2, w.opts(closed_start(), None)).await;
    let _ = timeout(WITHIN, b.wait_listen_outcome()).await;
    let res = timeout(WITHIN, b.connect(&c.local_addr(), PORT)).await;

    assert!(
        matches!(res, Ok(Err(_))),
        "without a table and start nodes C cannot be found: {:?}",
        res.as_ref().map(|r| r.as_ref().map(|_| ()))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dead_table_does_not_block_bootstrap() {
    let w = setup("dead").await;
    let c = w.serving_node(3, w.opts(w.cluster.bootstrap(), None)).await;
    let dead: Vec<SocketAddrV4> = ["127.0.0.1:9", "127.0.0.1:7", "127.0.0.1:19"]
        .iter()
        .map(|s| s.parse().expect("address"))
        .collect();
    std::fs::write(&w.file, render(&dead, unix_now())).expect("write the table");

    let b = w
        .node(2, w.opts(w.cluster.bootstrap(), Some(w.file.clone())))
        .await;
    timeout(WITHIN, b.wait_listen_outcome())
        .await
        .expect("B's announcement hung")
        .expect("B's announcement");
    let res = timeout(WITHIN, b.connect(&c.local_addr(), PORT)).await;

    assert!(
        matches!(res, Ok(Ok(_))),
        "dead entries must not stop the start nodes from working: {:?}",
        res.as_ref().map(|r| r.as_ref().map(|_| ()))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stale_table_is_ignored_and_kept() {
    let w = setup("stale").await;
    let live: SocketAddrV4 = w.cluster.bootstrap()[0].parse().expect("cluster address");
    let stale = render(&[live], unix_now() - TTL.as_secs() - 60);
    std::fs::write(&w.file, &stale).expect("write the table");

    let a = w
        .node(1, w.opts(closed_start(), Some(w.file.clone())))
        .await;
    a.close().await;

    let after = std::fs::read(&w.file).expect("the table is still there");
    assert_eq!(
        after,
        stale.as_bytes(),
        "a stale table is not used, so the node learns nothing and writes nothing"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn close_writes_table() {
    let w = setup("close").await;
    let a = w
        .node(1, w.opts(w.cluster.bootstrap(), Some(w.file.clone())))
        .await;
    timeout(WITHIN, a.wait_listen_outcome())
        .await
        .expect("A's announcement hung")
        .expect("A's announcement");

    a.close().await;

    let text = std::fs::read_to_string(&w.file).expect("close must write the table file");
    let nodes = parse(&text, unix_now(), TTL).expect("the file parses");
    assert!(!nodes.is_empty(), "the table has nodes");
}

// ---------- setup ----------

/// A loopback cluster, the table file path in a scratch directory, and the nodes bound so far.
struct World {
    cluster: Cluster,
    dir: PathBuf,
    file: PathBuf,
}

async fn setup(name: &str) -> World {
    let cluster = cluster(3).await;
    let dir = scratch(name);
    let file = dir.join("dht-nodes.txt");
    World { cluster, dir, file }
}

impl World {
    fn opts(&self, bootstrap: Vec<String>, table_file: Option<PathBuf>) -> HyperOpts {
        HyperOpts {
            bootstrap: Some(bootstrap),
            table_file,
            ..Default::default()
        }
    }

    async fn node(&self, seed: u8, opts: HyperOpts) -> Arc<HyperTransport> {
        HyperTransport::bind(secret(seed), opts)
            .await
            .expect("bind a node")
    }

    /// A node that serves `AUTH` and `PORT` and whose announcement is done.
    async fn serving_node(&self, seed: u8, opts: HyperOpts) -> Arc<HyperTransport> {
        let node = self.node(seed, opts).await;
        node.serve(&[AUTH, PORT]);
        timeout(WITHIN, node.wait_listen_outcome())
            .await
            .expect("announcement hung")
            .expect("announcement");
        node
    }

    /// Starts a node with the table file on the cluster, lets it learn the neighbours and stops it:
    /// the file then holds that node's view of the cluster.
    async fn saved_table_of_node(&self, seed: u8) {
        let opts = self.opts(self.cluster.bootstrap(), Some(self.file.clone()));
        let node = self.node(seed, opts).await;
        timeout(WITHIN, node.wait_listen_outcome())
            .await
            .expect("announcement hung")
            .expect("announcement");
        node.close().await;
        assert!(
            self.file.exists(),
            "the first node must leave the table file"
        );
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn closed_start() -> Vec<String> {
    vec![CLOSED_START.to_string()]
}

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-dht-table-net-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("could not create the test's temporary directory");
    dir
}
