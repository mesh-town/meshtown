//! Tests of the table writer (`saver.rs`): the write decision, the periodic task and the final write.

use super::*;
use crate::transport::net_hyper::dht_table::{parse, TTL};
use crate::transport::net_hyper::test_cluster::{cluster, Cluster};
use std::net::SocketAddrV4;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use townjump_dht::{Dht, DhtFactory};
use townjump_dht_peeroxide::{PeeroxideDht, PeeroxideFactory};

const NOW: u64 = 5_000_000;
const DAY: u64 = 24 * 3600;
const TICK: Duration = Duration::from_millis(200);
const LONG: Duration = Duration::from_secs(3600);
const WITHIN: Duration = Duration::from_secs(15);

// ---------- the write decision (`should_write`) ----------

#[test]
fn first_nonempty_table_is_written() {
    assert!(should_write(None, &[addr("1.2.3.4:5")], NOW));
}

#[test]
fn empty_table_is_never_written() {
    let last = Written::new(&[addr("1.2.3.4:5")], NOW);

    assert!(!should_write(None, &[], NOW), "nothing was written yet");
    assert!(
        !should_write(Some(&last), &[], NOW + 2 * DAY),
        "an empty table must not replace a good file, even a stale one"
    );
}

#[test]
fn same_set_is_skipped() {
    let last = Written::new(&[addr("1.2.3.4:5"), addr("6.7.8.9:10")], NOW);
    let reordered = [addr("6.7.8.9:10"), addr("1.2.3.4:5")];

    assert!(!should_write(Some(&last), &reordered, NOW + 600));
}

#[test]
fn same_set_is_rewritten_after_refresh() {
    let last = Written::new(&[addr("1.2.3.4:5")], NOW);
    let same = [addr("1.2.3.4:5")];

    assert!(
        !should_write(Some(&last), &same, NOW + DAY - 1),
        "just before the refresh time"
    );
    assert!(
        should_write(Some(&last), &same, NOW + DAY),
        "the stamp must not go stale while the set stays the same"
    );
}

#[test]
fn changed_set_is_written() {
    let last = Written::new(&[addr("1.2.3.4:5")], NOW);

    assert!(
        should_write(Some(&last), &[addr("1.2.3.4:6")], NOW + 1),
        "other node"
    );
    assert!(
        should_write(
            Some(&last),
            &[addr("1.2.3.4:5"), addr("6.7.8.9:10")],
            NOW + 1
        ),
        "node added"
    );
}

// ---------- the periodic task, `finish`, `Drop` ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn periodic_write_appears_without_close() {
    let f = setup("periodic").await;
    let _saver = TableSaver::start_with(f.node.clone(), f.path.clone(), TICK, TICK);

    f.then_file_appears().await;

    assert!(!f.read_nodes().is_empty(), "the table has nodes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn finish_writes_and_is_idempotent() {
    let f = setup("finish").await;
    let saver = TableSaver::start_with(f.node.clone(), f.path.clone(), LONG, LONG);
    assert!(!f.path.exists(), "no tick has happened yet");

    saver.finish().await;
    assert!(!f.read_nodes().is_empty(), "finish writes the table");

    saver.finish().await;
    assert!(f.path.exists(), "the second finish changes nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_writes_after_finish() {
    // Writes fail while the directory is missing, so a live task would keep trying every tick.
    let mut f = setup("after-finish").await;
    f.use_missing_dir();
    let saver = TableSaver::start_with(f.node.clone(), f.path.clone(), TICK, TICK);
    sleep(TICK * 2).await;
    saver.finish().await;

    f.create_parent_dir();
    sleep(TICK * 4).await;

    assert!(!f.path.exists(), "a finished saver must not tick again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dropped_saver_stops_writing() {
    let mut f = setup("dropped").await;
    f.use_missing_dir();
    let saver = TableSaver::start_with(f.node.clone(), f.path.clone(), TICK, TICK);
    sleep(TICK * 2).await;

    drop(saver);
    sleep(TICK).await; // a write that was already running ends
    f.create_parent_dir();
    sleep(TICK * 4).await;

    assert!(
        !f.path.exists(),
        "a dropped saver must not leave a zombie writer"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn write_failure_is_logged_once_and_saver_keeps_going() {
    let mut f = setup("failure").await;
    f.use_missing_dir();
    let before = crate::node::output::log_page(0, 1).last_seq;

    let _saver = TableSaver::start_with(f.node.clone(), f.path.clone(), TICK, TICK);
    sleep(TICK * 6).await;
    let lines = f.journal_about(before);
    assert_eq!(
        lines.len(),
        1,
        "one line for several failed ticks: {lines:?}"
    );
    assert!(
        lines[0].contains("not written"),
        "the line says what failed: {lines:?}"
    );

    f.create_parent_dir();
    f.then_file_appears().await;
    assert_eq!(
        f.journal_about(before).len(),
        1,
        "no new lines once it works"
    );
}

// ---------- setup ----------

/// A node on a loopback cluster whose table is not empty, and a scratch directory.
struct Fixture {
    _cluster: Cluster,
    node: Arc<PeeroxideDht>,
    dir: PathBuf,
    path: PathBuf,
}

async fn setup(name: &str) -> Fixture {
    let cluster = cluster(3).await;
    let factory = PeeroxideFactory {
        bootstrap: Some(cluster.bootstrap()),
        ..Default::default()
    };
    let node = Arc::new(factory.create(None).await.expect("DHT node"));
    timeout(WITHIN, wait_for_table(&node))
        .await
        .expect("the node learned no neighbours");
    let dir = scratch(name);
    let path = dir.join("dht-nodes.txt");
    Fixture {
        _cluster: cluster,
        node,
        dir,
        path,
    }
}

impl Fixture {
    /// Points the table file into a directory that does not exist yet: every write fails.
    fn use_missing_dir(&mut self) {
        self.path = self.dir.join("not-yet").join("dht-nodes.txt");
    }

    fn create_parent_dir(&self) {
        std::fs::create_dir_all(self.path.parent().expect("parent")).expect("create the directory");
    }

    async fn then_file_appears(&self) {
        let appeared = async {
            while !self.path.exists() {
                sleep(Duration::from_millis(50)).await;
            }
        };
        timeout(WITHIN, appeared)
            .await
            .unwrap_or_else(|_| panic!("{} did not appear", self.path.display()));
    }

    fn read_nodes(&self) -> Vec<SocketAddrV4> {
        let text = std::fs::read_to_string(&self.path).expect("the table file");
        parse(&text, crate::transport::net_mainline::unix_now(), TTL).expect("a valid table file")
    }

    /// Texts of the `hyper` journal lines after `mark` that name this fixture's file.
    fn journal_about(&self, mark: u64) -> Vec<String> {
        let shown = self.path.display().to_string();
        crate::node::output::log_page(mark, 100_000)
            .lines
            .into_iter()
            .filter(|l| l.tag == "hyper" && l.text.contains(&shown))
            .map(|l| l.text)
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn wait_for_table(node: &PeeroxideDht) {
    while node.to_array(None).await.map_or(true, |t| t.is_empty()) {
        sleep(Duration::from_millis(50)).await;
    }
}

fn addr(s: &str) -> SocketAddrV4 {
    s.parse().expect("test address")
}

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-dht-saver-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("could not create the test's temporary directory");
    dir
}
