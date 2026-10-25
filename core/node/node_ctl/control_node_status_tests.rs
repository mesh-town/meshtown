//! `NodeCtl::snapshot` status for a node that is up: the phase decides `Starting`/`Stopping`, and for
//! `Up` with a cached snapshot the link is judged by the live member count read at the call, not by the
//! cached one. The cache is "frozen" after `attach` (the generation grows, so the background
//! collection no longer writes it) and replaced by a snapshot with chosen `members.count` and
//! `relay.connected`, so each test controls the stale values exactly.

use super::*;

use std::sync::Arc;

use tokio::time::sleep;

use crate::members::network_key::NetworkKey;
use crate::node::{NodeConfig, RunningNode};
use crate::web_gui::{NodeControl, NodeStatus};

use super::snapshot::{collect, Collected, DocCache};

/// The time within which two loopback nodes see each other.
const CONNECT_LIMIT: Duration = Duration::from_secs(10);

mod up_without_cache {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn up_without_cache_is_starting_with_a_live_node() {
        // given: an attached node whose cache is empty and cannot be refilled
        let fx = setup(Opts::default()).await;
        // when: the snapshot is read
        let snap = fx.snapshot();
        // then: the node is still starting, whatever the live link looks like
        assert_eq!(snap.node.status, NodeStatus::Starting);
        fx.shutdown().await;
    }
}

mod up_with_stale_cache {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stale_empty_cache_with_a_live_member_is_online() {
        // given: two connected nodes, the cache still says "no members"
        let fx = setup(Opts {
            peer: true,
            cache: Some(Cached {
                members: 0,
                relay: false,
            }),
        })
        .await;
        // when: the snapshot is read
        let snap = fx.snapshot();
        // then: the live member makes the node online and the block shows it
        assert!(snap.members.count >= 1, "the members block is live");
        assert_eq!(snap.node.status, NodeStatus::Online);
        fx.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stale_cache_with_the_member_gone_is_offline() {
        // given: a lone node, the cache still counts one member
        let fx = setup(Opts {
            peer: false,
            cache: Some(Cached {
                members: 1,
                relay: false,
            }),
        })
        .await;
        // when: the snapshot is read
        let snap = fx.snapshot();
        // then: no live member and no relay, so the node is offline
        assert_eq!(snap.members.count, 0, "the members block is live");
        assert_eq!(snap.node.status, NodeStatus::Offline);
        fx.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cached_relay_with_no_live_member_is_online() {
        // given: a lone node, the cache has no members but a connected relay
        let fx = setup(Opts {
            peer: false,
            cache: Some(Cached {
                members: 0,
                relay: true,
            }),
        })
        .await;
        // when: the snapshot is read
        let snap = fx.snapshot();
        // then: the relay alone keeps the node online
        assert_eq!(snap.members.count, 0);
        assert_eq!(snap.node.status, NodeStatus::Online);
        fx.shutdown().await;
    }
}

mod stopping {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stopping_with_a_live_member_stays_stopping() {
        // given: two connected nodes and a cache that would say online
        let fx = setup(Opts {
            peer: true,
            cache: Some(Cached {
                members: 1,
                relay: false,
            }),
        })
        .await;
        // when: the process is asked to stop
        fx.ctl.stop_on_os_signal();
        // then: the phase wins over the link
        let snap = fx.snapshot();
        assert!(
            snap.members.count >= 1,
            "precondition: a live member is present"
        );
        assert_eq!(snap.node.status, NodeStatus::Stopping);
        fx.shutdown().await;
    }
}

/// What the frozen cache claims about the link.
#[derive(Clone, Copy)]
struct Cached {
    members: u64,
    relay: bool,
}

/// Preconditions of a test: whether a second node is connected and what the frozen cache holds
/// (`None` — no cache at all).
#[derive(Default)]
struct Opts {
    peer: bool,
    cache: Option<Cached>,
}

struct Fixture {
    ctl: Arc<NodeCtl>,
    nodes: Vec<RunningNode>,
}

impl Fixture {
    fn snapshot(&self) -> NodeSnapshot {
        self.ctl.snapshot()
    }

    async fn shutdown(self) {
        for node in self.nodes {
            node.shutdown().await;
        }
    }
}

/// Starts the node (and, with `peer`, a second one it knows), attaches the first to a controller and
/// freezes the controller's cache as `opts.cache` says.
async fn setup(opts: Opts) -> Fixture {
    let key = NetworkKey::generate().unwrap();
    let node = start_test_node(key.clone()).await;
    let mut nodes = vec![node.clone()];
    if opts.peer {
        let other = start_test_node(key).await;
        connect(&node, &other).await;
        nodes.push(other);
    }
    let ctl = NodeCtl::new(&Settings::default());
    ctl.attach(&node);
    freeze_cache(&ctl, &node, opts.cache).await;
    Fixture { ctl, nodes }
}

async fn start_test_node(key: NetworkKey) -> RunningNode {
    crate::node::start_node(NodeConfig::for_loopback_test(key))
        .await
        .unwrap()
}

/// Introduces `other` to `node` and waits until `node` counts a member.
async fn connect(node: &RunningNode, other: &RunningNode) {
    node.introduce(crate::transport::net::PeerAddr::new(other.id))
        .await;
    let deadline = Instant::now() + CONNECT_LIMIT;
    while node.members.count() < 1 {
        assert!(
            Instant::now() < deadline,
            "the nodes did not connect within the limit"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

/// Stops the background collection from writing the cache (the generation grows) and puts a snapshot
/// with the given link facts in its place, or clears it.
async fn freeze_cache(ctl: &NodeCtl, node: &RunningNode, cached: Option<Cached>) {
    let frozen = match cached {
        Some(c) => Some(collected_with(node, c).await),
        None => None,
    };
    let mut st = ctl
        .ctl_shared
        .ctl_state
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    st.snapshot_gen += 1;
    st.cache = frozen;
}

async fn collected_with(node: &RunningNode, cached: Cached) -> Collected {
    let mut c = collect(node, None, "direct_and_relay", &mut DocCache::default()).await;
    c.snap.members.count = cached.members as _;
    c.snap.transport.relay.connected = cached.relay;
    c
}
